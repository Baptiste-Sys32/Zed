mod app_server;
mod profiles;

use app_server::{CodexBridge, CodexThreadSummary};
use gpui::{
    Action, App, Context, EventEmitter, FocusHandle, Focusable, IntoElement, ParentElement, Render,
    SharedString, Window, actions, div,
};
use profiles::{CodexProfile, CodexProfileStore};
use std::path::PathBuf;
use ui::{
    Button, ButtonCommon, Color, Icon, IconButton, IconName, IconSize, Label, LabelCommon,
    LabelSize, Tooltip, prelude::*,
};
use workspace::{
    Workspace,
    item::{Item, ItemEvent, TabContentParams},
};

actions!(
    codex,
    [
        /// Opens the native Codex panel.
        TogglePanel,
        /// Starts a new Codex thread.
        NewThread,
        /// Opens Codex session resume.
        ResumeThread,
        /// Shows Codex status.
        ShowStatus,
        /// Opens Codex account profiles.
        ToggleAccountSelector,
    ]
);

pub fn init(cx: &mut App) {
    cx.observe_new(|workspace: &mut Workspace, window, _cx| {
        let Some(_window) = window else {
            return;
        };

        workspace.register_action(|workspace, _: &TogglePanel, window, cx| {
            open_codex_panel(workspace, window, cx);
        });
    })
    .detach();
}

struct CodexPanel {
    focus_handle: FocusHandle,
    cwd: Option<PathBuf>,
    profile_store: CodexProfileStore,
    active_profile: CodexProfile,
    bridge: Option<CodexBridge>,
    show_profiles: bool,
    status: SharedString,
    transcript: Vec<SharedString>,
    sessions: Vec<CodexThreadSummary>,
}

impl CodexPanel {
    fn new(cx: &mut Context<Self>) -> Self {
        let profile_store = CodexProfileStore::new();
        let active_profile = profile_store.active_profile();
        Self {
            focus_handle: cx.focus_handle(),
            cwd: std::env::current_dir().ok(),
            profile_store,
            active_profile,
            bridge: None,
            show_profiles: false,
            status: "Codex app-server is idle.".into(),
            transcript: vec![
                "Native Codex panel".into(),
                "Use /new, /resume, /status, or Ctrl+Alt+P for Profiles.".into(),
            ],
            sessions: Vec::new(),
        }
    }

    fn ensure_bridge(&mut self) -> anyhow::Result<&mut CodexBridge> {
        if self.bridge.is_none() {
            self.bridge = Some(CodexBridge::spawn(
                "codex",
                self.active_profile.id.clone(),
                self.active_profile.home.clone(),
            )?);
        }
        Ok(self.bridge.as_mut().expect("bridge is initialized"))
    }

    fn run_slash_command(&mut self, command: &str, cx: &mut Context<Self>) {
        let command = command.trim();
        let result = match command {
            "/new" => self.start_new_thread(),
            "/resume" => self.open_resume_picker(),
            "/status" => self.show_status(),
            other if other.starts_with("/resume ") => {
                self.resume_thread(other.trim_start_matches("/resume ").trim())
            }
            _ => {
                self.transcript
                    .push(format!("Unknown Codex command: {command}").into());
                Ok(())
            }
        };

        if let Err(error) = result {
            self.status = format!("Codex error: {error}").into();
            self.transcript.push(self.status.clone());
        }

        cx.notify();
    }

    fn start_new_thread(&mut self) -> anyhow::Result<()> {
        let cwd = self.cwd.clone().map(|cwd| cwd.display().to_string());
        let bridge = self.ensure_bridge()?;
        let response = bridge.thread_start(cwd)?;
        self.status = format!("Started Codex thread {}", response.thread_id).into();
        self.transcript.push("/new".into());
        self.transcript.push(self.status.clone());
        Ok(())
    }

    fn open_resume_picker(&mut self) -> anyhow::Result<()> {
        let bridge = self.ensure_bridge()?;
        self.sessions = bridge.thread_list(30)?;
        self.show_profiles = false;
        self.status = format!("Loaded {} Codex sessions.", self.sessions.len()).into();
        self.transcript.push("/resume".into());
        self.transcript.push(self.status.clone());
        Ok(())
    }

    fn resume_thread(&mut self, thread_id: &str) -> anyhow::Result<()> {
        let cwd = self.cwd.clone().map(|cwd| cwd.display().to_string());
        let bridge = self.ensure_bridge()?;
        bridge.thread_resume(thread_id, cwd)?;
        self.status = format!("Resumed Codex thread {thread_id}").into();
        self.transcript.push(format!("/resume {thread_id}").into());
        self.transcript.push(self.status.clone());
        Ok(())
    }

    fn show_status(&mut self) -> anyhow::Result<()> {
        let profile = self.active_profile.clone();
        let account = self
            .ensure_bridge()?
            .account_read()
            .unwrap_or_else(|error| format!("Unavailable ({error})"));
        self.status = format!(
            "Codex profile: {} | home: {} | account: {}",
            profile.label,
            profile.home.display(),
            account
        )
        .into();
        self.transcript.push("/status".into());
        self.transcript.push(self.status.clone());
        Ok(())
    }

    fn switch_profile(&mut self, profile: CodexProfile, cx: &mut Context<Self>) {
        self.active_profile = profile.clone();
        self.profile_store.set_active_profile(profile.id.clone());
        self.bridge = None;
        self.show_profiles = false;
        self.status = format!("Switched Codex profile to {}", profile.label).into();
        self.transcript.push(self.status.clone());
        cx.notify();
    }

    fn toggle_profiles(&mut self, cx: &mut Context<Self>) {
        self.show_profiles = !self.show_profiles;
        cx.notify();
    }
}

impl EventEmitter<ItemEvent> for CodexPanel {}

impl Focusable for CodexPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Item for CodexPanel {
    type Event = ItemEvent;

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString {
        "Codex".into()
    }

    fn tab_icon(&self, _window: &Window, _cx: &App) -> Option<Icon> {
        Some(Icon::new(IconName::AiOpenAi))
    }

    fn tab_content(&self, params: TabContentParams, _: &Window, cx: &App) -> gpui::AnyElement {
        h_flex()
            .gap_1()
            .child(Icon::new(IconName::AiOpenAi).size(IconSize::XSmall))
            .child(Label::new(self.tab_content_text(0, cx)).color(params.text_color()))
            .into_any_element()
    }

    fn telemetry_event_text(&self) -> Option<&'static str> {
        Some("Native Codex Panel Opened")
    }

    fn show_toolbar(&self) -> bool {
        false
    }

    fn to_item_events(event: &Self::Event, f: &mut dyn FnMut(ItemEvent)) {
        f(*event)
    }
}

impl Render for CodexPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let profiles = self.profile_store.profiles();

        v_flex()
            .key_context("CodexPanel")
            .on_action(cx.listener(|this, _: &ToggleAccountSelector, _, cx| {
                this.toggle_profiles(cx);
            }))
            .on_action(cx.listener(|this, _: &NewThread, _, cx| {
                this.run_slash_command("/new", cx);
            }))
            .on_action(cx.listener(|this, _: &ResumeThread, _, cx| {
                this.run_slash_command("/resume", cx);
            }))
            .on_action(cx.listener(|this, _: &ShowStatus, _, cx| {
                this.run_slash_command("/status", cx);
            }))
            .size_full()
            .bg(cx.theme().colors().editor_background)
            .child(
                h_flex()
                    .h_10()
                    .px_2()
                    .gap_1()
                    .border_b_1()
                    .border_color(cx.theme().colors().border_variant)
                    .child(Icon::new(IconName::AiOpenAi).size(IconSize::Small))
                    .child(Label::new("Codex").size(LabelSize::Small))
                    .child(div().flex_1())
                    .child(
                        Button::new("codex-new", "New")
                            .label_size(LabelSize::Small)
                            .on_click(|_, window, cx| {
                                window.dispatch_action(NewThread.boxed_clone(), cx);
                            }),
                    )
                    .child(
                        Button::new("codex-resume", "Resume")
                            .label_size(LabelSize::Small)
                            .on_click(|_, window, cx| {
                                window.dispatch_action(ResumeThread.boxed_clone(), cx);
                            }),
                    )
                    .child(
                        Button::new("codex-status", "Status")
                            .label_size(LabelSize::Small)
                            .on_click(|_, window, cx| {
                                window.dispatch_action(ShowStatus.boxed_clone(), cx);
                            }),
                    )
                    .child(
                        IconButton::new("codex-profiles", IconName::UserGroup)
                            .icon_size(IconSize::Small)
                            .tooltip(|_window, cx| {
                                Tooltip::for_action(
                                    "Profiles",
                                    &ToggleAccountSelector::default(),
                                    cx,
                                )
                            })
                            .on_click(|_, window, cx| {
                                window.dispatch_action(ToggleAccountSelector.boxed_clone(), cx);
                            }),
                    ),
            )
            .when(self.show_profiles, |this| {
                this.child(
                    v_flex()
                        .p_2()
                        .gap_1()
                        .border_b_1()
                        .border_color(cx.theme().colors().border_variant)
                        .child(
                            Label::new("Profiles")
                                .size(LabelSize::Small)
                                .color(Color::Muted),
                        )
                        .children(profiles.into_iter().enumerate().map(|(ix, profile)| {
                            let active = profile.id == self.active_profile.id;
                            Button::new(
                                ("codex-profile", ix),
                                if active {
                                    format!("{} (active)", profile.label)
                                } else {
                                    profile.label.clone()
                                },
                            )
                            .label_size(LabelSize::Small)
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.switch_profile(profile.clone(), cx);
                                },
                            ))
                        })),
                )
            })
            .child(
                v_flex()
                    .flex_1()
                    .p_3()
                    .gap_2()
                    .child(Label::new(self.status.clone()).size(LabelSize::Small))
                    .children(self.transcript.iter().map(|line| {
                        Label::new(line.clone())
                            .size(LabelSize::Small)
                            .color(Color::Muted)
                    }))
                    .when(!self.sessions.is_empty(), |this| {
                        this.child(Label::new("Recent Sessions").size(LabelSize::Small))
                            .children(self.sessions.iter().enumerate().map(|(ix, session)| {
                                Button::new(
                                    ("codex-session", ix),
                                    session.title.clone().unwrap_or_else(|| {
                                        session.thread_id.chars().take(12).collect()
                                    }),
                                )
                                .label_size(LabelSize::Small)
                                .on_click(cx.listener({
                                    let thread_id = session.thread_id.clone();
                                    move |this, _, _, cx| {
                                        this.run_slash_command(&format!("/resume {thread_id}"), cx);
                                    }
                                }))
                            }))
                    }),
            )
    }
}

fn open_codex_panel(workspace: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let panel = cx.new(|cx| CodexPanel::new(cx));
    workspace.add_item_to_active_pane(Box::new(panel), None, true, window, cx);
}
