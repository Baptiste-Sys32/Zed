use anyhow::{Context as _, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

type PendingMap = Arc<Mutex<HashMap<u64, mpsc::Sender<anyhow::Result<Value>>>>>;

#[derive(Debug, Deserialize)]
pub struct CodexThreadStartResponse {
    #[serde(rename = "threadId")]
    pub thread_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CodexThreadSummary {
    #[serde(rename = "threadId", alias = "id")]
    pub thread_id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
}

pub struct CodexBridge {
    #[allow(dead_code)]
    profile_id: String,
    #[allow(dead_code)]
    child: Mutex<Child>,
    stdin: Mutex<Option<ChildStdin>>,
    pending: PendingMap,
    next_id: AtomicU64,
    alive: Arc<AtomicBool>,
}

impl CodexBridge {
    pub fn spawn(bin: &str, profile_id: String, profile_home: PathBuf) -> anyhow::Result<Self> {
        std::fs::create_dir_all(&profile_home)
            .with_context(|| format!("create {}", profile_home.display()))?;

        let mut child = Command::new(bin)
            .arg("app-server")
            .arg("--listen")
            .arg("stdio://")
            .env("CODEX_HOME", &profile_home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("start `{bin} app-server`"))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Codex app-server stdin was unavailable"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Codex app-server stdout was unavailable"))?;

        let bridge = Self {
            profile_id,
            child: Mutex::new(child),
            stdin: Mutex::new(Some(stdin)),
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: AtomicU64::new(1),
            alive: Arc::new(AtomicBool::new(true)),
        };
        bridge.spawn_stdout_reader(stdout);
        bridge.request(
            "initialize",
            json!({
                "clientInfo": {
                    "name": "codex_zed",
                    "title": "Zed",
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "capabilities": {
                    "experimentalApi": true,
                    "requestAttestation": false,
                },
            }),
        )?;
        bridge.notify("initialized", json!({}))?;
        Ok(bridge)
    }

    pub fn thread_start(
        &mut self,
        cwd: Option<String>,
    ) -> anyhow::Result<CodexThreadStartResponse> {
        let value = self.request(
            "thread/start",
            json!({
                "cwd": cwd,
                "model": null,
                "approvalPolicy": null,
                "sandbox": null,
                "serviceName": "zed",
                "threadSource": "user",
                "sessionStartSource": null,
            }),
        )?;
        serde_json::from_value(value).context("decode Codex thread/start response")
    }

    pub fn thread_resume(&mut self, thread_id: &str, cwd: Option<String>) -> anyhow::Result<()> {
        self.request(
            "thread/resume",
            json!({
                "threadId": thread_id,
                "cwd": cwd,
                "model": null,
                "approvalPolicy": null,
                "sandbox": null,
            }),
        )?;
        Ok(())
    }

    pub fn thread_list(&mut self, limit: u64) -> anyhow::Result<Vec<CodexThreadSummary>> {
        let value = self.request(
            "thread/list",
            json!({
                "limit": limit,
                "sortKey": "updated_at",
                "sortDirection": "desc",
                "archived": false,
            }),
        )?;
        if let Some(threads) = value.get("threads") {
            return serde_json::from_value(threads.clone()).context("decode Codex thread list");
        }
        if let Some(data) = value.get("data") {
            return serde_json::from_value(data.clone()).context("decode Codex thread list");
        }
        Ok(Vec::new())
    }

    pub fn account_read(&mut self) -> anyhow::Result<String> {
        let value = self.request("account/read", json!({ "refreshToken": false }))?;
        Ok(value
            .get("account")
            .and_then(|account| {
                account
                    .get("email")
                    .or_else(|| account.get("label"))
                    .or_else(|| account.get("type"))
            })
            .and_then(Value::as_str)
            .unwrap_or("Signed in")
            .to_string())
    }

    fn request(&self, method: &str, params: Value) -> anyhow::Result<Value> {
        if !self.alive.load(Ordering::Relaxed) {
            return Err(anyhow!("Codex app-server is not running"));
        }

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel();
        self.pending
            .lock()
            .map_err(|_| anyhow!("Codex pending map was poisoned"))?
            .insert(id, tx);

        let message = json!({ "id": id, "method": method, "params": params });
        if let Err(error) = self.write_message(&message) {
            let _ = self.pending.lock().map(|mut pending| pending.remove(&id));
            return Err(error);
        }

        match rx.recv_timeout(REQUEST_TIMEOUT) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let _ = self.pending.lock().map(|mut pending| pending.remove(&id));
                Err(anyhow!(
                    "Timed out waiting for Codex response to `{method}`"
                ))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(anyhow!("Codex app-server response channel closed"))
            }
        }
    }

    fn notify(&self, method: &str, params: Value) -> anyhow::Result<()> {
        self.write_message(&json!({ "method": method, "params": params }))
    }

    fn write_message(&self, value: &Value) -> anyhow::Result<()> {
        let mut guard = self
            .stdin
            .lock()
            .map_err(|_| anyhow!("Codex app-server stdin was poisoned"))?;
        let stdin = guard
            .as_mut()
            .ok_or_else(|| anyhow!("Codex app-server stdin is closed"))?;
        serde_json::to_writer(&mut *stdin, value)?;
        stdin.write_all(b"\n")?;
        stdin.flush()?;
        Ok(())
    }

    fn spawn_stdout_reader(&self, stdout: impl std::io::Read + Send + 'static) {
        let pending = self.pending.clone();
        let alive = self.alive.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if line.trim().is_empty() {
                    continue;
                }
                let Ok(value) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                if let Some(id) = value.get("id").and_then(Value::as_u64)
                    && let Ok(mut pending) = pending.lock()
                    && let Some(tx) = pending.remove(&id)
                {
                    let result = if let Some(error) = value.get("error") {
                        Err(anyhow!("Codex app-server error: {error}"))
                    } else {
                        Ok(value.get("result").cloned().unwrap_or(Value::Null))
                    };
                    let _ = tx.send(result);
                }
            }

            alive.store(false, Ordering::Relaxed);
            if let Ok(mut pending) = pending.lock() {
                for (_, tx) in pending.drain() {
                    let _ = tx.send(Err(anyhow!("Codex app-server exited")));
                }
            }
        });
    }
}
