//! The interactive styleguide, laid out like React Styleguidist: a sidebar of
//! components with a filter, and a page per component with its description,
//! props table and live examples, each with knobs and "View code".
//!
//! [`Gallery`] is an ordinary view: [`run`] hosts it in a native window, or,
//! built for wasm32 with the `web` feature, full-page in the browser, where
//! `#/Component` opens a component and `#!/example-id?knob=value` opens one
//! example in isolation (what rustdoc previews embed as their playground).

use crate::{Example, Frame, Knobs, Styleguide, preview::Canvas};
use gpui::{
    AnyView, App, AppContext as _, Bounds, Context, Entity, FocusHandle, Focusable, FontWeight,
    HighlightStyle, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    StyledText, WeakEntity, Window, WindowBounds, WindowOptions, div, prelude::FluentBuilder as _,
    px, rgb, size,
};
use gpui_styleguidist_manifest::{KnobControl, KnobEntry, KnobValue};
use std::{collections::BTreeMap, ops::Range, rc::Rc};

const MONO: &str = "JetBrains Mono";

/// React Styleguidist's default theme.
struct Palette {
    base: Hsla,
    light: Hsla,
    lighter: Hsla,
    link: Hsla,
    link_hover: Hsla,
    border: Hsla,
    sidebar: Hsla,
    code_background: Hsla,
    selected: Hsla,
    white: Hsla,
}

fn palette() -> Palette {
    Palette {
        base: rgb(0x333333).into(),
        light: rgb(0x767676).into(),
        lighter: rgb(0xcccccc).into(),
        link: rgb(0x1978c8).into(),
        link_hover: rgb(0xf28a25).into(),
        border: rgb(0xe8e8e8).into(),
        sidebar: rgb(0xf5f5f5).into(),
        code_background: rgb(0xf5f5f5).into(),
        selected: rgb(0xe8f1fa).into(),
        white: rgb(0xffffff).into(),
    }
}

/// Where the styleguide is pointed: a component page, or one example alone.
#[derive(Clone, Debug, PartialEq)]
pub enum Route {
    Component(SharedString),
    Isolated {
        id: SharedString,
        knobs: BTreeMap<String, String>,
    },
}

struct Group {
    name: SharedString,
    description: SharedString,
    examples: Vec<usize>,
}

fn groups(guide: &Styleguide) -> Vec<Group> {
    let mut groups: Vec<Group> = guide
        .components
        .iter()
        .map(|(name, description)| Group {
            name: name.clone(),
            description: description.clone(),
            examples: Vec::new(),
        })
        .collect();
    for (index, example) in guide.examples.iter().enumerate() {
        let name = example.group();
        match groups.iter_mut().find(|group| group.name == name) {
            Some(group) => group.examples.push(index),
            None => groups.push(Group {
                name,
                description: SharedString::default(),
                examples: vec![index],
            }),
        }
    }
    groups
}

pub struct Gallery {
    guide: Rc<Styleguide>,
    groups: Rc<Vec<Group>>,
    route: Route,
    theme: usize,
    content: Option<AnyView>,
    filter: String,
    filtering: bool,
    filter_focus: FocusHandle,
}

impl Gallery {
    pub fn new(
        guide: Rc<Styleguide>,
        route: Option<Route>,
        theme: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let groups = Rc::new(groups(&guide));
        let theme = theme
            .and_then(|theme| guide.themes.iter().position(|name| name == theme))
            .unwrap_or(0);
        if let Some(name) = guide.themes.get(theme).cloned() {
            guide.apply_theme(&name, cx);
        }
        let route = route
            .and_then(|route| resolve(&guide, &groups, route))
            .or_else(|| {
                groups
                    .first()
                    .map(|group| Route::Component(group.name.clone()))
            })
            .unwrap_or(Route::Component(SharedString::default()));
        let mut this = Self {
            guide,
            groups,
            route: route.clone(),
            theme,
            content: None,
            filter: String::new(),
            filtering: false,
            filter_focus: cx.focus_handle(),
        };
        this.navigate(route, window, cx);
        this
    }

    fn navigate(&mut self, route: Route, window: &mut Window, cx: &mut Context<Self>) {
        let gallery = cx.weak_entity();
        self.content = match &route {
            Route::Component(name) => self
                .groups
                .iter()
                .position(|group| &group.name == name)
                .map(|group| {
                    let guide = self.guide.clone();
                    let groups = self.groups.clone();
                    cx.new(|cx| ComponentPage::new(guide, groups, group, gallery, window, cx))
                        .into()
                }),
            Route::Isolated { id, knobs } => {
                let index = self
                    .guide
                    .examples
                    .iter()
                    .position(|example| example.id() == id);
                index.map(|index| {
                    let guide = self.guide.clone();
                    let knobs = Knobs::with_raw(knobs.clone());
                    cx.new(|cx| ExampleBlock::new(guide, index, knobs, true, gallery, window, cx))
                        .into()
                })
            }
        };
        #[cfg(all(feature = "web", target_family = "wasm"))]
        crate::web::link(&route, self.theme_name());
        self.route = route;
        cx.notify();
    }

    #[cfg_attr(not(all(feature = "web", target_family = "wasm")), allow(dead_code))]
    fn theme_name(&self) -> Option<&str> {
        self.guide
            .themes
            .get(self.theme)
            .map(|theme| theme.as_ref())
    }

    fn set_theme(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.theme = index;
        let theme = self.guide.themes[index].clone();
        self.guide.apply_theme(&theme, cx);
        self.navigate(self.route.clone(), window, cx);
        cx.refresh_windows();
    }

    fn filter_key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.filtering {
            return;
        }
        if edit_text(&mut self.filter, event, &mut self.filtering) {
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = palette();
        let filter = self.filter.to_lowercase();
        let current = match &self.route {
            Route::Component(name) => Some(name.clone()),
            Route::Isolated { .. } => None,
        };
        div()
            .id("styleguide-sidebar")
            .w(px(240.))
            .h_full()
            .flex_none()
            .overflow_y_scroll()
            .bg(palette.sidebar)
            .border_r_1()
            .border_color(palette.border)
            .child(
                div()
                    .px_4()
                    .py_4()
                    .border_b_1()
                    .border_color(palette.border)
                    .text_size(px(18.))
                    .text_color(palette.base)
                    .child(self.guide.name.clone()),
            )
            .child(
                div().px_4().pt_4().pb_2().child(
                    div()
                        .id("styleguide-filter")
                        .track_focus(&self.filter_focus)
                        .on_key_down(cx.listener(Self::filter_key))
                        .px_2()
                        .py_1()
                        .rounded(px(4.))
                        .border_1()
                        .border_color(if self.filtering {
                            palette.link
                        } else {
                            palette.border
                        })
                        .bg(palette.white)
                        .text_sm()
                        .map(|this| match (self.filter.is_empty(), self.filtering) {
                            (true, false) => {
                                this.text_color(palette.lighter).child("Filter by name")
                            }
                            (_, true) => this
                                .text_color(palette.base)
                                .child(format!("{}▏", self.filter)),
                            (false, false) => {
                                this.text_color(palette.base).child(self.filter.clone())
                            }
                        })
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.filtering = true;
                            window.focus(&this.filter_focus, cx);
                            cx.notify();
                        })),
                ),
            )
            .child(
                div().px_4().pb_4().flex().flex_col().children(
                    self.groups
                        .iter()
                        .filter(|group| {
                            filter.is_empty()
                                || group.name.to_lowercase().contains(&filter)
                                || group.examples.iter().any(|index| {
                                    self.guide.examples[*index]
                                        .display_title()
                                        .to_lowercase()
                                        .contains(&filter)
                                })
                        })
                        .enumerate()
                        .map(|(index, group)| {
                            let selected = current.as_ref() == Some(&group.name);
                            let name = group.name.clone();
                            div()
                                .id(("styleguide-component", index))
                                .py_1()
                                .text_size(px(15.))
                                .text_color(palette.link)
                                .when(selected, |this| this.font_weight(FontWeight::BOLD))
                                .hover(|this| this.text_color(palette.link_hover))
                                .child(group.name.clone())
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.navigate(Route::Component(name.clone()), window, cx)
                                }))
                        }),
                ),
            )
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = palette();
        let back = match &self.route {
            Route::Isolated { id, .. } => self
                .guide
                .examples
                .iter()
                .find(|example| example.id() == id)
                .map(Example::group),
            Route::Component(_) => None,
        };
        div()
            .flex()
            .items_center()
            .justify_end()
            .gap_3()
            .when_some(back, |this, group| {
                this.child(
                    link("styleguide-back", format!("← {group}"), palette.link).on_click(
                        cx.listener(move |this, _, window, cx| {
                            this.navigate(Route::Component(group.clone()), window, cx)
                        }),
                    ),
                )
            })
            .when(self.guide.themes.len() > 1, |this| {
                this.child(segmented(
                    "styleguide-theme",
                    self.guide.themes.iter().cloned(),
                    self.theme,
                    cx.listener(|this, index: &usize, window, cx| {
                        this.set_theme(*index, window, cx)
                    }),
                ))
            })
    }
}

/// Maps a route onto what exists: a bare example id opens its component.
fn resolve(guide: &Styleguide, groups: &[Group], route: Route) -> Option<Route> {
    match route {
        Route::Component(name) => {
            if groups.iter().any(|group| group.name == name) {
                Some(Route::Component(name))
            } else {
                guide
                    .examples
                    .iter()
                    .find(|example| example.id() == &name)
                    .map(|example| Route::Component(example.group()))
            }
        }
        Route::Isolated { id, knobs } => guide
            .examples
            .iter()
            .any(|example| example.id() == &id)
            .then_some(Route::Isolated { id, knobs }),
    }
}

impl Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = palette();
        let isolated = matches!(self.route, Route::Isolated { .. });
        div()
            .size_full()
            .flex()
            .bg(palette.white)
            .text_color(palette.base)
            .text_size(px(15.))
            .when(!isolated, |this| this.child(self.sidebar(cx)))
            .child(
                div()
                    .id("styleguide-main")
                    .flex_1()
                    .h_full()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .max_w(px(1000.))
                            .px(px(if isolated { 16. } else { 32. }))
                            .py(px(if isolated { 12. } else { 16. }))
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(self.toolbar(cx))
                            .map(|this| match self.content.clone() {
                                Some(content) => this.child(content),
                                None => this.child(
                                    div()
                                        .text_color(palette.light)
                                        .child("No examples registered."),
                                ),
                            }),
                    ),
            )
    }
}

/// One component: heading, introduction, props table and every example.
struct ComponentPage {
    groups: Rc<Vec<Group>>,
    group: usize,
    blocks: Vec<Entity<ExampleBlock>>,
    props_open: bool,
    settle_frames: u8,
}

impl ComponentPage {
    fn new(
        guide: Rc<Styleguide>,
        groups: Rc<Vec<Group>>,
        group: usize,
        gallery: WeakEntity<Gallery>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let blocks = groups[group]
            .examples
            .iter()
            .map(|index| {
                let guide = guide.clone();
                let gallery = gallery.clone();
                cx.new(|cx| {
                    ExampleBlock::new(guide, *index, Knobs::default(), false, gallery, window, cx)
                })
            })
            .collect();
        Self {
            groups,
            group,
            blocks,
            props_open: true,
            settle_frames: 2,
        }
    }

    /// Every knob the component's examples read, in first-read order.
    fn props(&self, cx: &App) -> Vec<KnobEntry> {
        let mut props: Vec<KnobEntry> = Vec::new();
        for block in &self.blocks {
            for spec in block.read(cx).knobs.specs() {
                if !props.iter().any(|prop| prop.name == spec.name) {
                    props.push(spec);
                }
            }
        }
        props
    }
}

impl Render for ComponentPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Examples register their knobs while they render, after this view;
        // render again once they have, so the props table is complete.
        if self.settle_frames > 0 {
            self.settle_frames -= 1;
            window.request_animation_frame();
        }
        let palette = palette();
        let group = &self.groups[self.group];
        let props = self.props(cx);
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_size(px(36.))
                    .line_height(px(44.))
                    .text_color(palette.base)
                    .child(group.name.clone()),
            )
            .when(!group.description.is_empty(), |this| {
                this.child(
                    div()
                        .text_size(px(16.))
                        .line_height(px(24.))
                        .child(group.description.clone()),
                )
            })
            .when(!props.is_empty(), |this| {
                this.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            tab(
                                "styleguide-props-toggle",
                                "Props & methods",
                                self.props_open,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.props_open = !this.props_open;
                                cx.notify();
                            })),
                        )
                        .when(self.props_open, |this| this.child(props_table(&props))),
                )
            })
            .children(self.blocks.iter().cloned())
    }
}

fn props_table(props: &[KnobEntry]) -> impl IntoElement {
    let palette = palette();
    let cell = || div().py_2().pr_4().flex_1();
    div()
        .flex()
        .flex_col()
        .text_size(px(13.))
        .child(
            div()
                .flex()
                .border_b_1()
                .border_color(palette.border)
                .text_color(palette.light)
                .child(cell().child("Prop name"))
                .child(cell().child("Type"))
                .child(cell().child("Default")),
        )
        .children(props.iter().map(|prop| {
            let (kind, values) = match &prop.control {
                KnobControl::Bool => ("bool".to_string(), None),
                KnobControl::Select { options } => ("enum".to_string(), Some(options.join(" | "))),
                KnobControl::Text { samples } if samples.len() > 1 => (
                    "string".to_string(),
                    Some(format!(
                        "e.g. {}",
                        samples
                            .iter()
                            .map(|sample| format!("{sample:?}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )),
                ),
                KnobControl::Text { .. } => ("string".to_string(), None),
                KnobControl::Number { min, max, .. } => (format!("number ({min}..={max})"), None),
            };
            div()
                .flex()
                .border_b_1()
                .border_color(palette.border)
                .child(
                    cell()
                        .font_family(MONO)
                        .text_color(rgb(0x690000))
                        .child(prop.name.clone()),
                )
                .child(
                    cell()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .font_family(MONO)
                                .text_color(rgb(0x990055))
                                .child(kind),
                        )
                        .when_some(values, |this, values| {
                            this.child(div().text_color(palette.light).child(values))
                        }),
                )
                .child(cell().font_family(MONO).text_color(palette.light).child(
                    match &prop.default {
                        KnobValue::Text(text) => format!("{text:?}"),
                        value => value.key(),
                    },
                ))
        }))
}

/// One live example: its preview, knobs and source.
struct ExampleBlock {
    guide: Rc<Styleguide>,
    index: usize,
    knobs: Knobs,
    preview: AnyView,
    panel: Entity<KnobPanel>,
    isolated: bool,
    show_code: bool,
    show_knobs: bool,
    gallery: WeakEntity<Gallery>,
}

impl ExampleBlock {
    fn new(
        guide: Rc<Styleguide>,
        index: usize,
        knobs: Knobs,
        isolated: bool,
        gallery: WeakEntity<Gallery>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let block = cx.weak_entity();
        let panel = cx.new(|cx| KnobPanel {
            block,
            knobs: knobs.clone(),
            editing: None,
            focus: cx.focus_handle(),
        });
        let preview = build_preview(&guide, index, knobs.clone(), window, cx);
        Self {
            guide,
            index,
            knobs,
            preview,
            panel,
            isolated,
            show_code: false,
            show_knobs: true,
            gallery,
        }
    }

    fn example(&self) -> &Example {
        &self.guide.examples[self.index]
    }

    fn knobs_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.preview = build_preview(&self.guide, self.index, self.knobs.clone(), window, cx);
        // Keep an isolated example's URL in step, so it can be shared.
        #[cfg(all(feature = "web", target_family = "wasm"))]
        if self.isolated
            && let Some(gallery) = self.gallery.upgrade()
        {
            let knobs = self
                .knobs
                .specs()
                .iter()
                .filter_map(|spec| Some((spec.name.clone(), self.knobs.value(&spec.name)?.key())))
                .collect();
            let route = Route::Isolated {
                id: self.example().id().clone(),
                knobs,
            };
            crate::web::link(&route, gallery.read(cx).theme_name());
        }
        cx.notify();
    }
}

fn build_preview(
    guide: &Styleguide,
    index: usize,
    knobs: Knobs,
    window: &mut Window,
    cx: &mut App,
) -> AnyView {
    let example = &guide.examples[index];
    let content = (example.builder())(knobs, window, cx);
    let background = guide.background.clone();
    let fill = example.fixed_size().is_some();
    cx.new(|_| Canvas {
        content,
        background,
        fill,
    })
    .into()
}

impl Render for ExampleBlock {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = palette();
        let example = self.example();
        let id = example.id().clone();
        let source = example.source_text().clone();
        let has_knobs = !self.knobs.specs().is_empty();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .pt_4()
            .when(!self.isolated, |this| {
                this.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(20.))
                                .text_color(palette.base)
                                .child(example.display_title()),
                        )
                        .child(
                            link("styleguide-isolate", "Open isolated", palette.light).on_click(
                                cx.listener(move |this, _, window, cx| {
                                    let id = id.clone();
                                    this.gallery
                                        .update(cx, |gallery, cx| {
                                            let route = Route::Isolated {
                                                id,
                                                knobs: BTreeMap::new(),
                                            };
                                            gallery.navigate(route, window, cx)
                                        })
                                        .ok();
                                }),
                            ),
                        ),
                )
            })
            .when(!example.description_text().is_empty(), |this| {
                this.child(
                    div()
                        .text_color(palette.base)
                        .line_height(px(22.))
                        .child(example.description_text().clone()),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .border_1()
                    .border_color(palette.border)
                    .rounded(px(3.))
                    .child(
                        div()
                            .id("styleguide-preview")
                            .overflow_x_scroll()
                            .rounded_t(px(3.))
                            .child(self.preview.clone()),
                    )
                    .when(has_knobs || !source.is_empty(), |this| {
                        this.child(
                            div()
                                .flex()
                                .gap_4()
                                .px_4()
                                .py_2()
                                .border_t_1()
                                .border_color(palette.border)
                                .when(has_knobs, |this| {
                                    this.child(
                                        tab("styleguide-knobs-tab", "Knobs", self.show_knobs)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.show_knobs = !this.show_knobs;
                                                cx.notify();
                                            })),
                                    )
                                })
                                .when(!source.is_empty(), |this| {
                                    this.child(
                                        tab("styleguide-code-tab", "View code", self.show_code)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.show_code = !this.show_code;
                                                cx.notify();
                                            })),
                                    )
                                }),
                        )
                    })
                    // After the preview, so it renders once the preview has
                    // registered its knobs this frame.
                    .when(self.show_knobs, |this| this.child(self.panel.clone()))
                    .when(self.show_code && !source.is_empty(), |this| {
                        this.child(code_block(&source))
                    }),
            )
    }
}

fn code_block(source: &SharedString) -> impl IntoElement {
    let palette = palette();
    div()
        .id("styleguide-code")
        .overflow_x_scroll()
        .bg(palette.code_background)
        .border_t_1()
        .border_color(palette.border)
        .px_4()
        .py_3()
        .font_family(MONO)
        .text_size(px(13.))
        .line_height(px(20.))
        .text_color(palette.base)
        .child(StyledText::new(source.clone()).with_highlights(highlight(source)))
}

/// Live controls for one example's knobs.
struct KnobPanel {
    block: WeakEntity<ExampleBlock>,
    knobs: Knobs,
    editing: Option<String>,
    focus: FocusHandle,
}

impl Focusable for KnobPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl KnobPanel {
    fn set(&mut self, name: &str, value: KnobValue, window: &mut Window, cx: &mut Context<Self>) {
        self.knobs.set(name, value);
        self.changed(window, cx);
    }

    fn changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.notify();
        self.block
            .update(cx, |block, cx| block.knobs_changed(window, cx))
            .ok();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(name) = self.editing.clone() else {
            return;
        };
        let Some(KnobValue::Text(mut text)) = self.knobs.value(&name) else {
            return;
        };
        let mut editing = true;
        if edit_text(&mut text, event, &mut editing) {
            cx.stop_propagation();
            if editing {
                self.set(&name, KnobValue::Text(text), window, cx);
            } else {
                self.editing = None;
                cx.notify();
            }
        }
    }
}

/// Applies a key press to a plain text field. Returns whether it was handled;
/// clears `editing` on Enter, Escape or Tab.
fn edit_text(text: &mut String, event: &KeyDownEvent, editing: &mut bool) -> bool {
    let keystroke = &event.keystroke;
    match keystroke.key.as_str() {
        "escape" | "enter" | "tab" => {
            *editing = false;
            true
        }
        "backspace" => {
            text.pop();
            true
        }
        _ if keystroke.modifiers.control || keystroke.modifiers.platform => false,
        _ => match &keystroke.key_char {
            Some(chars) => {
                text.push_str(chars);
                true
            }
            None => false,
        },
    }
}

impl Render for KnobPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = palette();
        let specs = self.knobs.specs();
        div()
            .id("styleguide-knobs")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .when(!specs.is_empty(), |this| {
                this.border_t_1().border_color(palette.border).px_4().py_3()
            })
            .flex()
            .flex_col()
            .gap_2()
            .text_size(px(13.))
            .children(specs.iter().enumerate().map(|(index, spec)| {
                let name: SharedString = spec.name.clone().into();
                let value = self.knobs.value(&spec.name).unwrap_or(spec.default.clone());
                let control = match (&spec.control, value) {
                    (KnobControl::Bool, KnobValue::Bool(on)) => {
                        let name = name.clone();
                        toggle(("styleguide-knob", index), on)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.set(&name, KnobValue::Bool(!on), window, cx)
                            }))
                            .into_any_element()
                    }
                    (KnobControl::Select { options }, KnobValue::Text(current)) => {
                        let options = options.clone();
                        let labels: Vec<SharedString> =
                            options.iter().cloned().map(SharedString::from).collect();
                        let selected = options.iter().position(|option| *option == current);
                        let name = name.clone();
                        segmented(
                            ("styleguide-knob", index),
                            labels,
                            selected.unwrap_or(usize::MAX),
                            cx.listener(move |this, choice: &usize, window, cx| {
                                let value = KnobValue::Text(options[*choice].clone());
                                this.set(&name, value, window, cx)
                            }),
                        )
                        .into_any_element()
                    }
                    (KnobControl::Text { samples }, KnobValue::Text(text)) => {
                        let editing = self.editing.as_deref() == Some(spec.name.as_str());
                        let knob = spec.name.clone();
                        let samples = samples.clone();
                        let field = div()
                            .id(("styleguide-knob", index))
                            .min_w(px(220.))
                            .px_2()
                            .py_1()
                            .rounded(px(3.))
                            .border_1()
                            .border_color(if editing {
                                palette.link
                            } else {
                                palette.border
                            })
                            .bg(palette.white)
                            .font_family(MONO)
                            .child(if editing {
                                format!("{text}▏")
                            } else {
                                format!("{text:?}")
                            })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.editing = Some(knob.clone());
                                window.focus(&this.focus, cx);
                                cx.notify();
                            }));
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(field)
                            .when(samples.len() > 1, |this| {
                                let name = name.clone();
                                let labels: Vec<SharedString> = samples
                                    .iter()
                                    .map(|sample| match sample.as_str() {
                                        "" => "(empty)".into(),
                                        sample => sample.to_string().into(),
                                    })
                                    .collect();
                                let selected = samples.iter().position(|sample| *sample == text);
                                this.child(segmented(
                                    ("styleguide-knob-samples", index),
                                    labels,
                                    selected.unwrap_or(usize::MAX),
                                    cx.listener(move |this, choice: &usize, window, cx| {
                                        this.editing = None;
                                        let value = KnobValue::Text(samples[*choice].clone());
                                        this.set(&name, value, window, cx)
                                    }),
                                ))
                            })
                            .into_any_element()
                    }
                    (KnobControl::Number { min, max, step }, KnobValue::Number(number)) => {
                        let (min, max, step) = (*min, *max, *step);
                        let (less, more) = (name.clone(), name.clone());
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                button(("styleguide-knob-less", index), "−", false).on_click(
                                    cx.listener(move |this, _, window, cx| {
                                        let value = (number - step).max(min);
                                        this.set(&less, KnobValue::Number(value), window, cx)
                                    }),
                                ),
                            )
                            .child(
                                div()
                                    .min_w(px(48.))
                                    .font_family(MONO)
                                    .child(format!("{number}")),
                            )
                            .child(
                                button(("styleguide-knob-more", index), "+", false).on_click(
                                    cx.listener(move |this, _, window, cx| {
                                        let value = (number + step).min(max);
                                        this.set(&more, KnobValue::Number(value), window, cx)
                                    }),
                                ),
                            )
                            .into_any_element()
                    }
                    _ => div().child("unsupported value").into_any_element(),
                };
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .w(px(120.))
                            .flex_none()
                            .font_family(MONO)
                            .text_color(palette.light)
                            .child(name),
                    )
                    .child(control)
            }))
            .when(!specs.is_empty(), |this| {
                this.child(div().pt_1().child(
                    link("styleguide-knobs-reset", "Reset knobs", palette.link).on_click(
                        cx.listener(|this, _, window, cx| {
                            this.knobs.reset();
                            this.editing = None;
                            this.changed(window, cx);
                        }),
                    ),
                ))
            })
    }
}

/// Styleguidist's uppercase tab button.
fn tab(id: &'static str, label: &'static str, active: bool) -> gpui::Stateful<gpui::Div> {
    let palette = palette();
    div()
        .id(id)
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(if active { palette.base } else { palette.light })
        .when(active, |this| this.border_b_2().border_color(palette.link))
        .hover(|this| this.text_color(palette.link))
        .child(label.to_uppercase())
}

fn link(
    id: &'static str,
    label: impl Into<SharedString>,
    color: Hsla,
) -> gpui::Stateful<gpui::Div> {
    let palette = palette();
    div()
        .id(id)
        .text_size(px(13.))
        .text_color(color)
        .hover(|this| this.text_color(palette.link_hover))
        .child(label.into())
}

fn toggle(id: impl Into<gpui::ElementId>, on: bool) -> gpui::Stateful<gpui::Div> {
    let palette = palette();
    div()
        .id(id)
        .w(px(32.))
        .h(px(18.))
        .rounded_full()
        .p(px(2.))
        .flex()
        .when(on, |this| this.justify_end().bg(palette.link))
        .when(!on, |this| this.bg(palette.lighter))
        .child(div().size(px(14.)).rounded_full().bg(palette.white))
}

fn button(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    active: bool,
) -> gpui::Stateful<gpui::Div> {
    let palette = palette();
    div()
        .id(id)
        .px_2()
        .py_0p5()
        .rounded(px(3.))
        .border_1()
        .border_color(palette.border)
        .bg(palette.white)
        .when(active, |this| {
            this.bg(palette.link)
                .border_color(palette.link)
                .text_color(palette.white)
        })
        .when(!active, |this| this.hover(|this| this.bg(palette.selected)))
        .child(label.into())
}

fn segmented(
    id: impl Into<gpui::ElementId>,
    labels: impl IntoIterator<Item = SharedString>,
    selected: usize,
    on_select: impl Fn(&usize, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let on_select = Rc::new(on_select);
    let id: gpui::ElementId = id.into();
    div()
        .id(id.clone())
        .flex()
        .flex_wrap()
        .gap_1()
        .children(labels.into_iter().enumerate().map(|(index, label)| {
            let on_select = on_select.clone();
            button(
                (SharedString::from(format!("{id}-{index}")), 0usize),
                label,
                index == selected,
            )
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(move |_, window, cx| on_select(&index, window, cx))
        }))
}

/// A small Rust highlighter in the colors of Styleguidist's Prism theme.
fn highlight(source: &str) -> Vec<(Range<usize>, HighlightStyle)> {
    const KEYWORDS: &[&str] = &[
        "as", "async", "await", "break", "const", "continue", "crate", "else", "enum", "false",
        "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
        "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
        "use", "where", "while",
    ];
    let color = |hex: u32| HighlightStyle {
        color: Some(rgb(hex).into()),
        ..Default::default()
    };
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        let c = bytes[i];
        if c == b'/' && bytes.get(i + 1) == Some(&b'/') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            out.push((start..i, color(0x767676)));
        } else if c == b'"' {
            i += 1;
            while i < bytes.len() && bytes[i] != b'"' {
                i += if bytes[i] == b'\\' { 2 } else { 1 };
            }
            i = (i + 1).min(bytes.len());
            out.push((start..i, color(0x669900)));
        } else if c.is_ascii_digit() {
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'.' || bytes[i] == b'_')
            {
                i += 1;
            }
            out.push((start..i, color(0x990055)));
        } else if c.is_ascii_alphabetic() || c == b'_' {
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let word = &source[start..i];
            let next = bytes.get(i).copied();
            if KEYWORDS.contains(&word) {
                out.push((start..i, color(0x1673b1)));
            } else if next == Some(b'(') || next == Some(b'!') {
                out.push((start..i, color(0xdd4a68)));
            } else if word.starts_with(|c: char| c.is_ascii_uppercase()) {
                out.push((start..i, color(0x990055)));
            }
        } else {
            // Step over a whole UTF-8 character.
            i += source[i..].chars().next().map_or(1, char::len_utf8);
        }
    }
    out
}

/// Opens the styleguide in a new window through [`Styleguide::open_window`].
pub fn open(
    guide: Rc<Styleguide>,
    route: Option<Route>,
    theme: Option<String>,
    cx: &mut App,
) -> anyhow::Result<()> {
    // The browser window is the page, whatever its requested bounds.
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(1280.), px(860.)),
            cx,
        ))),
        ..Default::default()
    };
    let open_window = guide.open_window.clone();
    open_window(
        options,
        Box::new(move |window, cx| {
            let gallery = cx.new(|cx| Gallery::new(guide, route, theme.as_deref(), window, cx));
            cx.new(|_| Frame::new(gallery.into()))
        }),
        cx,
    )?;
    Ok(())
}

/// Runs the styleguide app: until its window closes natively; for the page's
/// lifetime on the web, where the route comes from the URL. `example` opens
/// that example's component (or that component, by name).
pub fn run(guide: Styleguide, example: Option<String>) {
    let guide = Rc::new(guide);
    #[allow(unused_mut)]
    let mut route = example.map(|id| Route::Component(id.into()));
    #[allow(unused_mut)]
    let mut theme = None;
    #[cfg(not(target_family = "wasm"))]
    let mut app = gpui_platform::application();
    #[cfg(all(feature = "web", target_family = "wasm"))]
    let mut app = crate::web::application();
    #[cfg(all(feature = "web", target_family = "wasm"))]
    if route.is_none() {
        (route, theme) = crate::web::current_link();
    }
    if let Some(assets) = guide.assets.clone() {
        app = app.with_assets(SharedAssets(assets));
    }
    app.run(move |cx| {
        guide.run_setup(cx);
        #[cfg(not(target_family = "wasm"))]
        cx.on_window_closed(|cx, _| cx.quit()).detach();
        if let Err(error) = open(guide, route, theme, cx) {
            crate::guide::log_error(&format!("{error:#}"));
            cx.quit();
        }
        cx.activate(true);
    });
}

struct SharedAssets(std::sync::Arc<dyn gpui::AssetSource>);

impl gpui::AssetSource for SharedAssets {
    fn load(&self, path: &str) -> gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        self.0.load(path)
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        self.0.list(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_rust_tokens() {
        let source = "let label = knobs.text(\"label\", \"Save\"); // hi";
        let spans: Vec<&str> = highlight(source)
            .into_iter()
            .map(|(range, _)| &source[range])
            .collect();
        assert_eq!(spans, ["let", "text", "\"label\"", "\"Save\"", "// hi"]);
    }
}
