use crate::Knobs;
use gpui::{
    AnyElement, AnyView, AnyWindowHandle, App, AppContext as _, AssetSource, Context, Entity, Hsla,
    IntoElement, ParentElement as _, Pixels, Render, SharedString, Size, Styled as _, Window,
    WindowOptions, div, px, size, white,
};
use std::{borrow::Cow, path::PathBuf, rc::Rc, sync::Arc};

pub(crate) type BuildFn = Rc<dyn Fn(Knobs, &mut Window, &mut App) -> AnyView>;
type RenderFn = Rc<dyn Fn(&Knobs, &mut Window, &mut App) -> AnyElement>;

/// One documented state of a component: a button's primary variant, an empty
/// table, a form with errors.
///
/// Ids are slash-separated paths (`button/primary`). The first segment groups
/// examples in the gallery unless [`Example::component`] names the group.
pub struct Example {
    id: SharedString,
    title: Option<SharedString>,
    component: Option<SharedString>,
    description: SharedString,
    source: SharedString,
    size: Option<Size<Pixels>>,
    build: BuildFn,
}

impl Example {
    /// A stateless example, rendered from scratch every frame.
    ///
    /// ```ignore
    /// Example::new("button/primary", |knobs, _, _| {
    ///     Button::new("ok")
    ///         .label(knobs.text("label", "Save"))
    ///         .disabled(knobs.bool("disabled", false))
    /// })
    /// ```
    pub fn new<E: IntoElement>(
        id: impl Into<SharedString>,
        render: impl Fn(&Knobs, &mut Window, &mut App) -> E + 'static,
    ) -> Self {
        let render: RenderFn =
            Rc::new(move |knobs, window, cx| render(knobs, window, cx).into_any_element());
        Self::from_build(
            id.into(),
            Rc::new(move |knobs, _, cx| {
                let render = render.clone();
                cx.new(|_| Stateless { knobs, render }).into()
            }),
        )
    }

    /// A stateful example built once per knob change, for components that need
    /// an entity such as `InputState`. Keep `knobs` in the view to read it in
    /// `render`.
    pub fn view<V: Render>(
        id: impl Into<SharedString>,
        build: impl Fn(Knobs, &mut Window, &mut App) -> Entity<V> + 'static,
    ) -> Self {
        Self::from_build(
            id.into(),
            Rc::new(move |knobs, window, cx| build(knobs, window, cx).into()),
        )
    }

    fn from_build(id: SharedString, build: BuildFn) -> Self {
        Self {
            id,
            title: None,
            component: None,
            description: SharedString::default(),
            source: SharedString::default(),
            size: None,
            build,
        }
    }

    /// The heading in the gallery. Defaults to the id's last segment.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// The component this example documents, such as `Button`.
    pub fn component(mut self, component: impl Into<SharedString>) -> Self {
        self.component = Some(component.into());
        self
    }

    /// Plain text shown under the title.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = description.into();
        self
    }

    /// The code shown under "View code". [`example!`](crate::example) fills it
    /// from the example's source.
    pub fn source(mut self, source: impl Into<SharedString>) -> Self {
        self.source = source.into();
        self
    }

    /// Renders snapshots at exactly this logical size instead of cropping to
    /// the content. Use it for components that fill their parent.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = Some(size(px(width), px(height)));
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    pub fn display_title(&self) -> SharedString {
        self.title.clone().unwrap_or_else(|| {
            self.id
                .rsplit('/')
                .next()
                .unwrap_or(&self.id)
                .to_string()
                .into()
        })
    }

    /// The gallery group: [`Example::component`], else the id's first segment.
    pub fn group(&self) -> SharedString {
        self.component.clone().unwrap_or_else(|| {
            self.id
                .split('/')
                .next()
                .unwrap_or(&self.id)
                .to_string()
                .into()
        })
    }

    #[cfg_attr(not(feature = "render"), allow(dead_code))]
    pub(crate) fn component_name(&self) -> Option<&SharedString> {
        self.component.as_ref()
    }

    pub fn description_text(&self) -> &SharedString {
        &self.description
    }

    pub fn source_text(&self) -> &SharedString {
        &self.source
    }

    pub(crate) fn fixed_size(&self) -> Option<Size<Pixels>> {
        self.size
    }

    pub(crate) fn builder(&self) -> BuildFn {
        self.build.clone()
    }
}

struct Stateless {
    knobs: Knobs,
    render: RenderFn,
}

impl Render for Stateless {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        (self.render)(&self.knobs, window, cx)
    }
}

/// The root content of every window the styleguide opens. [`Styleguide::open_window`]
/// receives a builder for it so an application can wrap it in its own root
/// view, such as gpui-kit's `Root`.
pub struct Frame {
    content: AnyView,
}

impl Frame {
    pub(crate) fn new(content: AnyView) -> Self {
        Self { content }
    }
}

impl Render for Frame {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.content.clone())
    }
}

/// Builds the window's [`Frame`]; pass it to the application's window helper.
pub type FrameBuilder = Box<dyn FnOnce(&mut Window, &mut App) -> Entity<Frame>>;

type SetupFn = Rc<dyn Fn(&mut App)>;
type ThemeFn = Rc<dyn Fn(&str, &mut App)>;

pub(crate) type OpenWindowFn =
    Rc<dyn Fn(WindowOptions, FrameBuilder, &mut App) -> anyhow::Result<AnyWindowHandle>>;

/// A component library's examples and how to host them.
///
/// ```ignore
/// fn main() {
///     gpui_styleguidist::main(
///         Styleguide::new("My components")
///             .assets(gpui_kit::assets::Assets)
///             .setup(gpui_kit::init)
///             .open_window(|options, frame, cx| Ok(gpui_kit::open_window(options, cx, frame)?.0))
///             .example(Example::new("button/primary", |_, _, _| Button::new("ok").label("Save"))),
///     );
/// }
/// ```
pub struct Styleguide {
    pub(crate) name: SharedString,
    pub(crate) examples: Vec<Example>,
    pub(crate) setup: Option<SetupFn>,
    pub(crate) open_window: OpenWindowFn,
    pub(crate) themes: Vec<SharedString>,
    pub(crate) apply_theme: Option<ThemeFn>,
    pub(crate) background: Rc<dyn Fn(&App) -> Hsla>,
    pub(crate) assets: Option<Arc<dyn AssetSource>>,
    pub(crate) fonts: Vec<Cow<'static, [u8]>>,
    pub(crate) playground_url: Option<String>,
    pub(crate) components: Vec<(SharedString, SharedString)>,
    pub(crate) crate_dir: Option<PathBuf>,
    pub(crate) variant_limit: usize,
}

impl Styleguide {
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            examples: Vec::new(),
            setup: None,
            open_window: Rc::new(|options, frame, cx| Ok(cx.open_window(options, frame)?.into())),
            themes: vec!["default".into()],
            apply_theme: None,
            background: Rc::new(|_| white()),
            assets: None,
            fonts: Vec::new(),
            playground_url: None,
            components: Vec::new(),
            crate_dir: None,
            variant_limit: 24,
        }
    }

    pub fn example(mut self, example: Example) -> Self {
        self.examples.push(example);
        self
    }

    pub fn examples(mut self, examples: impl IntoIterator<Item = Example>) -> Self {
        self.examples.extend(examples);
        self
    }

    /// The introduction shown at the top of a component's page, above its
    /// props and examples. Components appear in the order first described or
    /// first used by an example.
    pub fn describe(
        mut self,
        component: impl Into<SharedString>,
        description: impl Into<SharedString>,
    ) -> Self {
        self.components.push((component.into(), description.into()));
        self
    }

    /// Runs once per app before any window opens, e.g. `gpui_kit::init`.
    pub fn setup(mut self, setup: impl Fn(&mut App) + 'static) -> Self {
        self.setup = Some(Rc::new(setup));
        self
    }

    /// Opens every styleguide window, so the application can install its root
    /// view. Defaults to `cx.open_window`.
    pub fn open_window(
        mut self,
        open: impl Fn(WindowOptions, FrameBuilder, &mut App) -> anyhow::Result<AnyWindowHandle>
        + 'static,
    ) -> Self {
        self.open_window = Rc::new(open);
        self
    }

    /// Renders every example once per theme. `apply` switches the app to the
    /// named theme. The first theme is the default.
    pub fn themes<S: Into<SharedString>>(
        mut self,
        themes: impl IntoIterator<Item = S>,
        apply: impl Fn(&str, &mut App) + 'static,
    ) -> Self {
        self.themes = themes.into_iter().map(Into::into).collect();
        assert!(
            !self.themes.is_empty(),
            "a styleguide needs at least one theme"
        );
        self.apply_theme = Some(Rc::new(apply));
        self
    }

    /// The canvas color behind previews, usually the theme's background.
    pub fn background(mut self, background: impl Fn(&App) -> Hsla + 'static) -> Self {
        self.background = Rc::new(background);
        self
    }

    pub fn assets(mut self, assets: impl AssetSource) -> Self {
        self.assets = Some(Arc::new(assets));
        self
    }

    /// Font files to load before any window opens. The web has no system
    /// fonts, so a web styleguide must bundle whatever its theme names; the
    /// `web` feature already bundles IBM Plex Sans for GPUI's `.SystemUIFont`.
    pub fn fonts(mut self, fonts: impl IntoIterator<Item = Cow<'static, [u8]>>) -> Self {
        self.fonts.extend(fonts);
        self
    }

    /// The hosted wasm styleguide (`cargo styleguide build`). Rustdoc previews
    /// offer it as a live playground, opened at `<url>#!/<example id>`.
    pub fn playground_url(mut self, url: impl Into<String>) -> Self {
        self.playground_url = Some(url.into());
        self
    }

    /// Where `styleguide/` is written. Defaults to `CARGO_MANIFEST_DIR`, which
    /// `cargo run` sets to the crate being run.
    pub fn crate_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.crate_dir = Some(dir.into());
        self
    }

    /// Caps the knob combinations rendered per example and theme (default 24).
    /// Past it, snapshots cover the default plus each single-knob change.
    pub fn variant_limit(mut self, limit: usize) -> Self {
        self.variant_limit = limit.max(1);
        self
    }

    pub fn name(&self) -> &SharedString {
        &self.name
    }

    pub fn all_examples(&self) -> &[Example] {
        &self.examples
    }

    #[cfg_attr(not(feature = "render"), allow(dead_code))]
    pub(crate) fn output_dir(&self) -> PathBuf {
        self.crate_dir
            .clone()
            .or_else(|| std::env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("."))
            .join(gpui_styleguidist_manifest::DIR)
    }

    pub(crate) fn run_setup(&self, cx: &mut App) {
        #[allow(unused_mut)]
        let mut fonts = self.fonts.clone();
        #[cfg(all(feature = "web", target_family = "wasm"))]
        fonts.push(Cow::Borrowed(include_bytes!(
            "../fonts/IBMPlexSans-Regular.ttf"
        )));
        if !fonts.is_empty()
            && let Err(error) = cx.text_system().add_fonts(fonts)
        {
            log_error(&format!("loading fonts: {error:#}"));
        }
        if let Some(setup) = &self.setup {
            setup(cx);
        }
    }

    pub(crate) fn apply_theme(&self, theme: &str, cx: &mut App) {
        if let Some(apply) = &self.apply_theme {
            apply(theme, cx);
        }
    }
}

/// Reports to stderr natively and to the browser console on the web.
pub(crate) fn log_error(message: &str) {
    #[cfg(all(feature = "web", target_family = "wasm"))]
    web_sys::console::error_1(&format!("styleguide: {message}").into());
    #[cfg(not(all(feature = "web", target_family = "wasm")))]
    eprintln!("styleguide: {message}");
}
