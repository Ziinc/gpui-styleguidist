//! A small application component library built on gpui-kit, documented with
//! gpui-styleguidist.
//!
//! Every component below shows its rendered examples. Pick knob values under a
//! preview to see other snapshots, or open the live playground.
//!
//! - `cargo styleguide serve -p kit-demo` builds the wasm styleguide.
//! - `cargo run -p kit-demo --example styleguide` opens it natively.
//! - `cargo run -p kit-demo --example styleguide -- render` re-renders the
//!   snapshots these docs embed; `-- check` compares against them.
//!
#![doc = gpui_styleguidist::preview_html!("setting-row/list", variants = false)]

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    component::{
        ActiveTheme as _, Disableable as _, Sizable as _, Size, StyledExt as _, Theme, ThemeMode,
        button::{Button, ButtonVariant, ButtonVariants as _},
        h_flex,
        input::{Input, InputState},
        switch::Switch,
        v_flex,
    },
    *,
};
use gpui_styleguidist::{Styleguide, example};

/// What an [`ActionButton`] does, which decides its emphasis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    /// The one action a surface exists for.
    #[default]
    Primary,
    /// Secondary actions.
    Neutral,
    /// Actions that destroy data.
    Danger,
}

/// A labelled command button whose emphasis follows its [`Tone`].
#[gpui_styleguidist::preview("action-button/tones")]
#[derive(IntoElement)]
pub struct ActionButton {
    id: ElementId,
    label: SharedString,
    tone: Tone,
    disabled: bool,
    loading: bool,
    size: Size,
}

impl ActionButton {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            tone: Tone::default(),
            disabled: false,
            loading: false,
            size: Size::Medium,
        }
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Shows a spinner and ignores clicks while work runs.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for ActionButton {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        Button::new(self.id)
            .label(self.label)
            .with_variant(match self.tone {
                Tone::Primary => ButtonVariant::Primary,
                Tone::Neutral => ButtonVariant::Default,
                Tone::Danger => ButtonVariant::Danger,
            })
            .disabled(self.disabled)
            .loading(self.loading)
            .with_size(self.size)
    }
}

/// A settings row: a title, an explanation and a switch.
#[gpui_styleguidist::preview("setting-row/notifications")]
#[derive(IntoElement)]
pub struct SettingRow {
    id: ElementId,
    title: SharedString,
    description: Option<SharedString>,
    checked: bool,
    disabled: bool,
}

impl SettingRow {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: None,
            checked: false,
            disabled: false,
        }
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for SettingRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        h_flex()
            .w(px(360.))
            .gap_4()
            .justify_between()
            .child(
                v_flex()
                    .gap_0p5()
                    .child(div().font_medium().child(self.title))
                    .when_some(self.description, |this, description| {
                        this.child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(description),
                        )
                    }),
            )
            .child(
                Switch::new(self.id)
                    .checked(self.checked)
                    .disabled(self.disabled),
            )
    }
}

/// A search box with a placeholder. Stateful: it owns an [`InputState`].
#[gpui_styleguidist::preview("search-field/empty")]
pub struct SearchField {
    input: Entity<InputState>,
    small: bool,
}

impl SearchField {
    pub fn new(
        placeholder: &str,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(placeholder.to_string())
                .default_value(value.to_string())
        });
        Self {
            input,
            small: false,
        }
    }

    pub fn small(mut self, small: bool) -> Self {
        self.small = small;
        self
    }
}

impl Render for SearchField {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let small = self.small;
        div().w(px(280.)).child(
            Input::new(&self.input)
                .cleanable(true)
                .when(small, |this| this.small()),
        )
    }
}

#[cfg(not(target_family = "wasm"))]
fn assets() -> gpui_kit::assets::Assets {
    gpui_kit::assets::Assets
}

/// On the web, gpui-kit fetches icons on demand; this uses the copies its own
/// gallery publishes.
#[cfg(target_family = "wasm")]
fn assets() -> gpui_kit::assets::Assets {
    gpui_kit::assets::Assets::new("https://gpui-kit.com/gallery/")
}

/// This crate's styleguide: every example, both themes, rendered through
/// gpui-kit's window root. `STYLEGUIDE_PLAYGROUND_URL`, when set while
/// rendering, links rustdoc previews to the hosted wasm styleguide.
pub fn styleguide() -> Styleguide {
    let guide = Styleguide::new("Kit demo")
        .assets(assets())
        .setup(gpui_kit::init)
        .open_window(|options, frame, cx| Ok(gpui_kit::open_window(options, cx, frame)?.0))
        .themes(["light", "dark"], |theme, cx| {
            let mode = if theme == "dark" {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            };
            Theme::change(mode, None, cx);
        })
        .background(|cx| cx.theme().background)
        .describe(
            "ActionButton",
            "A labelled command button. Its tone says what the action does: Primary for the \
             one action a surface exists for, Neutral for the rest, Danger for actions that \
             destroy data.",
        )
        .describe(
            "SettingRow",
            "One preference in a settings list: a title, an optional explanation and a switch.",
        )
        .describe(
            "SearchField",
            "A search box that owns its InputState, so its examples are stateful views.",
        )
        .example(
            example!("action-button/tones", |knobs, _, _| {
                ActionButton::new(
                    "action",
                    knobs.text_samples("label", &["Save changes", "Delete"]),
                )
                .tone(knobs.pick(
                    "tone",
                    &[
                        ("primary", Tone::Primary),
                        ("neutral", Tone::Neutral),
                        ("danger", Tone::Danger),
                    ],
                    "primary",
                ))
                .size(knobs.pick(
                    "size",
                    &[
                        ("small", Size::Small),
                        ("medium", Size::Medium),
                        ("large", Size::Large),
                    ],
                    "medium",
                ))
                .disabled(knobs.bool("disabled", false))
                .loading(knobs.bool("loading", false))
            })
            .component("ActionButton")
            .title("Playground")
            .description("Every prop as a knob.")
            .state("Default", [])
            .state("Disabled", [("disabled", "true")])
            .state("Loading", [("loading", "true")])
            .state("Destructive", [("tone", "danger"), ("label", "Delete")]),
        )
        .example(
            example!("action-button/all-tones", |_, _, _| {
                h_flex()
                    .gap_2()
                    .child(ActionButton::new("primary", "Publish").tone(Tone::Primary))
                    .child(ActionButton::new("neutral", "Save draft").tone(Tone::Neutral))
                    .child(ActionButton::new("danger", "Delete").tone(Tone::Danger))
            })
            .component("ActionButton")
            .title("Tones side by side")
            .description("Use Primary once per surface."),
        )
        .example(
            example!("setting-row/notifications", |knobs, _, _| {
                SettingRow::new("notifications", knobs.text("title", "Notifications"))
                    .description(knobs.text("description", "Send me a summary every morning."))
                    .checked(knobs.bool("checked", true))
                    .disabled(knobs.bool("disabled", false))
            })
            .component("SettingRow")
            .title("Notifications")
            .state("On", [])
            .state("Off", [("checked", "false")])
            .state("Managed", [("disabled", "true")]),
        )
        .example(
            example!("setting-row/list", |_, _, _| {
                v_flex()
                    .gap_4()
                    .child(
                        SettingRow::new("sync", "Sync")
                            .description("Keep files on every device.")
                            .checked(true),
                    )
                    .child(SettingRow::new("beta", "Beta features").checked(false))
                    .child(
                        SettingRow::new("admin", "Admin mode")
                            .description("Managed by your organization.")
                            .disabled(true),
                    )
            })
            .component("SettingRow")
            .title("A settings list"),
        )
        .example(
            example!(view "search-field/empty", |knobs, window, cx| {
                cx.new(|cx| {
                    SearchField::new(
                        &knobs.text("placeholder", "Search files…"),
                        &knobs.text_samples("value", &["", "invoice"]),
                        window,
                        cx,
                    )
                    .small(knobs.bool("small", false))
                })
            })
            .component("SearchField")
            .title("Empty and filled")
            .description("Changing a knob rebuilds the view and its InputState. Click it and type.")
            .state("Empty", [])
            .state("Filled", [("value", "invoice")])
            .state("Compact", [("small", "true")]),
        );
    match std::env::var("STYLEGUIDE_PLAYGROUND_URL") {
        Ok(url) if !url.is_empty() => guide.playground_url(url),
        _ => guide,
    }
}
