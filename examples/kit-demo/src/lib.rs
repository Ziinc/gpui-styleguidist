//! A small application component library built on gpui-kit, documented with
//! gpui-styleguidist.
//!
//! - `cargo run -p kit-demo --example styleguide` opens the gallery natively.
//! - `cargo styleguide serve -p kit-demo` builds it to wasm and serves it.
//! - `cargo run -p kit-demo --example styleguide -- check` compares the
//!   examples against the snapshots committed in `styleguide/`.

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
use gpui_styleguidist::{Example, Knobs, Styleguide};

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

impl Tone {
    pub const ALL: [(&'static str, Tone); 3] = [
        ("primary", Tone::Primary),
        ("neutral", Tone::Neutral),
        ("danger", Tone::Danger),
    ];
}

/// A labelled command button whose emphasis follows its [`Tone`].
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
pub struct SearchField {
    input: Entity<InputState>,
    knobs: Knobs,
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
            knobs: Knobs::default(),
        }
    }
}

impl Render for SearchField {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let small = self.knobs.bool("small", false);
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
/// gpui-kit's window root.
pub fn styleguide() -> Styleguide {
    Styleguide::new("Kit demo")
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
        .example(
            Example::new("action-button/tones", |knobs, _, _| {
                let tone = knobs.select("tone", &["primary", "neutral", "danger"], "primary");
                let tone = Tone::ALL
                    .iter()
                    .find(|(name, _)| *name == tone)
                    .map_or(Tone::Primary, |(_, tone)| *tone);
                let size = match knobs.select("size", &["small", "medium", "large"], "medium") {
                    "small" => Size::Small,
                    "large" => Size::Large,
                    _ => Size::Medium,
                };
                ActionButton::new(
                    "action",
                    knobs.text_samples("label", &["Save changes", "Delete"]),
                )
                .tone(tone)
                .size(size)
                .disabled(knobs.bool("disabled", false))
                .loading(knobs.bool("loading", false))
            })
            .component("ActionButton")
            .title("Tones")
            .description("Use Primary once per surface, and Danger only for destructive actions."),
        )
        .example(
            Example::new("setting-row/notifications", |knobs, _, _| {
                SettingRow::new("notifications", knobs.text("title", "Notifications"))
                    .description(knobs.text("description", "Send me a summary every morning."))
                    .checked(knobs.bool("checked", true))
                    .disabled(knobs.bool("disabled", false))
            })
            .component("SettingRow")
            .title("Notifications"),
        )
        .example(
            Example::view("search-field/empty", |knobs, window, cx| {
                let placeholder = knobs.text("placeholder", "Search files…");
                let value = knobs.text_samples("value", &["", "invoice"]);
                cx.new(|cx| {
                    let mut field = SearchField::new(&placeholder, &value, window, cx);
                    field.knobs = knobs.clone();
                    field
                })
            })
            .component("SearchField")
            .title("Empty and filled")
            .description(
                "A stateful example: changing a knob rebuilds the view and its InputState.",
            ),
        )
}
