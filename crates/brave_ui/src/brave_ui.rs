mod runtime;

use crate::runtime::BraveRuntime;
use gpui::{
    Action, App, Context, EventEmitter, FocusHandle, Focusable, IntoElement, ParentElement, Render,
    SharedString, Window, actions, div,
};
use ui::{
    Button, ButtonCommon, Color, Divider, Icon, IconButton, IconName, IconSize, Label, LabelCommon,
    LabelSize, Tooltip, prelude::*,
};
use workspace::{
    HideStatusItem, StatusItemView, Workspace,
    item::{Item, ItemEvent, ItemHandle, TabContentParams},
};

actions!(
    brave,
    [
        /// Opens the embedded Brave browser panel.
        TogglePanel,
    ]
);

pub fn init(cx: &mut App) {
    cx.observe_new(|workspace: &mut Workspace, window, _cx| {
        let Some(_window) = window else {
            return;
        };

        workspace.register_action(|workspace, _: &TogglePanel, window, cx| {
            open_brave_panel(workspace, window, cx);
        });
    })
    .detach();
}

pub struct BraveStatusButton;

impl BraveStatusButton {
    pub fn new() -> Self {
        Self
    }
}

impl Render for BraveStatusButton {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .gap_0p5()
            .child(Divider::vertical().color(ui::DividerColor::Border))
            .child(
                IconButton::new("brave-status-icon", IconName::Brave)
                    .icon_size(IconSize::Small)
                    .tooltip(|_window, cx| Tooltip::for_action("Open Brave", &TogglePanel, cx))
                    .on_click(|_, window, cx| {
                        window.dispatch_action(TogglePanel.boxed_clone(), cx);
                    }),
            )
    }
}

impl StatusItemView for BraveStatusButton {
    fn set_active_pane_item(
        &mut self,
        _: Option<&dyn ItemHandle>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
    }

    fn hide_setting(&self, _: &App) -> Option<HideStatusItem> {
        None
    }
}

struct BravePanel {
    focus_handle: FocusHandle,
    location: SharedString,
    runtime: BraveRuntime,
}

impl BravePanel {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            location: "https://search.brave.com".into(),
            runtime: BraveRuntime::detect(),
        }
    }
}

impl EventEmitter<ItemEvent> for BravePanel {}

impl Focusable for BravePanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Item for BravePanel {
    type Event = ItemEvent;

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString {
        "Brave".into()
    }

    fn tab_icon(&self, _window: &Window, _cx: &App) -> Option<Icon> {
        Some(Icon::new(IconName::Brave))
    }

    fn tab_content(&self, params: TabContentParams, _: &Window, cx: &App) -> gpui::AnyElement {
        h_flex()
            .gap_1()
            .child(Icon::new(IconName::Brave).size(IconSize::XSmall))
            .child(Label::new(self.tab_content_text(0, cx)).color(params.text_color()))
            .into_any_element()
    }

    fn telemetry_event_text(&self) -> Option<&'static str> {
        Some("Brave Panel Opened")
    }

    fn show_toolbar(&self) -> bool {
        false
    }

    fn to_item_events(event: &Self::Event, f: &mut dyn FnMut(ItemEvent)) {
        f(*event)
    }
}

impl Render for BravePanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let executable = self
            .runtime
            .executable
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "Not found".to_string());
        let user_data_dir = self
            .runtime
            .user_data_dir
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "Unavailable".to_string());
        let launch_ready = self.runtime.launch_command(0, &self.location).is_some();

        v_flex()
            .size_full()
            .bg(cx.theme().colors().editor_background)
            .child(
                h_flex()
                    .h_10()
                    .px_2()
                    .gap_1()
                    .border_b_1()
                    .border_color(cx.theme().colors().border_variant)
                    .child(IconButton::new("brave-back", IconName::ArrowLeft).disabled(true))
                    .child(IconButton::new("brave-forward", IconName::ArrowRight).disabled(true))
                    .child(IconButton::new("brave-reload", IconName::RotateCw).disabled(true))
                    .child(
                        div()
                            .flex_1()
                            .h_7()
                            .px_2()
                            .rounded_sm()
                            .border_1()
                            .border_color(cx.theme().colors().border_variant)
                            .child(
                                Label::new(self.location.clone())
                                    .size(LabelSize::Small)
                                    .color(Color::Muted),
                            ),
                    )
                    .child(Button::new("brave-open", "Open").disabled(true)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(Icon::new(IconName::Brave).size(IconSize::XLarge))
                    .child(Label::new("Embedded Brave"))
                    .child(
                        Label::new(format!("Executable: {executable}"))
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    )
                    .child(
                        Label::new(format!("User data: {user_data_dir}"))
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    )
                    .child(
                        Label::new(if launch_ready {
                            "Brave can be launched with a localhost DevTools endpoint from this profile."
                        } else {
                            "Set ZED_BRAVE_PATH or install Brave to enable embedded control."
                        })
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                    )
                    .child(
                        Label::new(
                            "The native panel is wired. The next slice attaches this surface to a local Brave tab over Chrome DevTools Protocol so cookies stay inside Brave.",
                        )
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                    ),
            )
    }
}

fn open_brave_panel(workspace: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let panel = cx.new(|cx| BravePanel::new(cx));
    workspace.add_item_to_active_pane(Box::new(panel), None, true, window, cx);
}
