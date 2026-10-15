//! The interactive gallery: every example with live knobs and themes.
//!
//! [`Gallery`] is an ordinary view: [`run`] hosts it in a native window, or,
//! built for wasm32 with the `web` feature, full-page in the browser.

use crate::manifest::{KnobControl, KnobValue};
use crate::{Frame, Knobs, Styleguide, preview::Canvas};
use gpui::{
    AnyView, App, AppContext as _, Bounds, Context, Entity, FocusHandle, Focusable, FontWeight,
    Hsla, InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton, ParentElement as _,
    Render, SharedString, StatefulInteractiveElement as _, Styled as _, WeakEntity, Window,
    WindowBounds, WindowOptions, div, prelude::FluentBuilder as _, px, rgb, size,
};
use std::rc::Rc;

struct Palette {
    chrome: Hsla,
    sidebar: Hsla,
    border: Hsla,
    text: Hsla,
    muted: Hsla,
    accent: Hsla,
    accent_text: Hsla,
    selected: Hsla,
}

const PALETTE: fn() -> Palette = || Palette {
    chrome: rgb(0xffffff).into(),
    sidebar: rgb(0xf6f6f7).into(),
    border: rgb(0xe2e2e5).into(),
    text: rgb(0x1d1d20).into(),
    muted: rgb(0x6e6e76).into(),
    accent: rgb(0x2f6fec).into(),
    accent_text: rgb(0xffffff).into(),
    selected: rgb(0xe4ecfd).into(),
};

pub struct Gallery {
    guide: Rc<Styleguide>,
    selected: usize,
    theme: usize,
    knobs: Knobs,
    preview: Option<AnyView>,
    panel: Entity<KnobPanel>,
}

impl Gallery {
    /// Opens on the example whose id is `initial`, else the first one.
    pub fn new(
        guide: Rc<Styleguide>,
        initial: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let selected = initial
            .and_then(|id| guide.examples.iter().position(|example| example.id() == id))
            .unwrap_or(0);
        let knobs = Knobs::default();
        let gallery = cx.weak_entity();
        let panel = cx.new(|cx| KnobPanel {
            gallery,
            knobs: knobs.clone(),
            editing: None,
            focus: cx.focus_handle(),
        });
        let mut this = Self {
            guide,
            selected,
            theme: 0,
            knobs,
            preview: None,
            panel,
        };
        if let Some(theme) = this.guide.themes.first().cloned() {
            this.guide.apply_theme(&theme, cx);
        }
        this.rebuild(window, cx);
        this
    }

    fn rebuild(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(example) = self.guide.examples.get(self.selected) else {
            self.preview = None;
            return;
        };
        let content = (example.builder())(self.knobs.clone(), window, cx);
        let background = self.guide.background.clone();
        let fill = example.fixed_size().is_some();
        self.preview = Some(
            cx.new(|_| Canvas {
                content,
                background,
                fill,
            })
            .into(),
        );
        cx.notify();
    }

    fn select(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = index;
        #[cfg(all(feature = "web", target_family = "wasm"))]
        crate::web::link_example(self.guide.examples[index].id());
        self.knobs = Knobs::default();
        self.panel.update(cx, |panel, _| {
            panel.knobs = self.knobs.clone();
            panel.editing = None;
        });
        self.rebuild(window, cx);
    }

    fn set_theme(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.theme = index;
        let theme = self.guide.themes[index].clone();
        self.guide.apply_theme(&theme, cx);
        self.rebuild(window, cx);
        cx.refresh_windows();
    }

    fn knob_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.rebuild(window, cx);
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = PALETTE();
        let mut groups: Vec<(SharedString, Vec<usize>)> = Vec::new();
        for (index, example) in self.guide.examples.iter().enumerate() {
            let group = example.group();
            match groups.iter_mut().find(|(name, _)| *name == group) {
                Some((_, items)) => items.push(index),
                None => groups.push((group, vec![index])),
            }
        }
        div()
            .id("styleguide-sidebar")
            .w(px(220.))
            .h_full()
            .flex_none()
            .overflow_y_scroll()
            .bg(palette.sidebar)
            .border_r_1()
            .border_color(palette.border)
            .py_3()
            .child(
                div()
                    .px_4()
                    .pb_3()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.guide.name.clone()),
            )
            .children(groups.into_iter().map(|(group, items)| {
                div()
                    .pb_2()
                    .child(
                        div()
                            .px_4()
                            .py_1()
                            .text_xs()
                            .text_color(palette.muted)
                            .child(group),
                    )
                    .children(items.into_iter().map(|index| {
                        let example = &self.guide.examples[index];
                        let selected = index == self.selected;
                        div()
                            .id(("styleguide-example", index))
                            .mx_2()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .text_sm()
                            .when(selected, |this| this.bg(palette.selected))
                            .hover(|this| this.bg(palette.selected.opacity(0.6)))
                            .child(example.display_title())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.select(index, window, cx)
                            }))
                    }))
            }))
    }

    fn header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = PALETTE();
        let example = &self.guide.examples[self.selected];
        div()
            .flex_none()
            .px_6()
            .py_4()
            .border_b_1()
            .border_color(palette.border)
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(example.display_title()),
                    )
                    .when(self.guide.themes.len() > 1, |this| {
                        this.child(segmented(
                            "styleguide-theme",
                            self.guide.themes.iter().cloned(),
                            self.theme,
                            cx.listener(|this, index: &usize, window, cx| {
                                this.set_theme(*index, window, cx)
                            }),
                        ))
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(palette.muted)
                    .child(example.id().clone()),
            )
            .when(!example.description_text().is_empty(), |this| {
                this.child(
                    div()
                        .pt_1()
                        .text_sm()
                        .child(example.description_text().clone()),
                )
            })
    }
}

impl Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = PALETTE();
        div()
            .size_full()
            .flex()
            .bg(palette.chrome)
            .text_color(palette.text)
            .text_sm()
            .child(self.sidebar(cx))
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .when(self.guide.examples.is_empty(), |this| {
                        this.child(div().p_6().child("No examples registered."))
                    })
                    .when(!self.guide.examples.is_empty(), |this| {
                        this.child(self.header(cx))
                            .child(
                                div()
                                    .id("styleguide-canvas")
                                    .flex_1()
                                    .overflow_scroll()
                                    .when_some(self.preview.clone(), |this, preview| {
                                        this.child(preview)
                                    }),
                            )
                            // After the preview, so it renders once the preview
                            // has registered its knobs this frame.
                            .child(self.panel.clone())
                    }),
            )
    }
}

/// Live controls for the selected example's knobs.
struct KnobPanel {
    gallery: WeakEntity<Gallery>,
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
        cx.notify();
        self.gallery
            .update(cx, |gallery, cx| gallery.knob_changed(window, cx))
            .ok();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(name) = self.editing.clone() else {
            return;
        };
        let Some(KnobValue::Text(mut text)) = self.knobs.value(&name) else {
            return;
        };
        let keystroke = &event.keystroke;
        match keystroke.key.as_str() {
            "escape" | "enter" | "tab" => {
                self.editing = None;
                cx.notify();
                return;
            }
            "backspace" => {
                text.pop();
            }
            _ if keystroke.modifiers.control || keystroke.modifiers.platform => return,
            _ => match &keystroke.key_char {
                Some(chars) => text.push_str(chars),
                None => return,
            },
        }
        cx.stop_propagation();
        self.set(&name, KnobValue::Text(text), window, cx);
    }
}

impl Render for KnobPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = PALETTE();
        let specs = self.knobs.specs();
        div()
            .id("styleguide-knobs")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .flex_none()
            .max_h(px(240.))
            .overflow_y_scroll()
            .border_t_1()
            .border_color(palette.border)
            .px_6()
            .py_3()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Knobs"))
                    .when(!specs.is_empty(), |this| {
                        this.child(button("styleguide-knobs-reset", "Reset", false).on_click(
                            cx.listener(|this, _, window, cx| {
                                this.knobs.reset();
                                this.editing = None;
                                cx.notify();
                                this.gallery
                                    .update(cx, |gallery, cx| gallery.knob_changed(window, cx))
                                    .ok();
                            }),
                        ))
                    }),
            )
            .when(specs.is_empty(), |this| {
                this.child(
                    div()
                        .text_color(palette.muted)
                        .child("This example has no knobs."),
                )
            })
            .children(specs.into_iter().enumerate().map(|(index, spec)| {
                let name: SharedString = spec.name.clone().into();
                let value = self.knobs.value(&spec.name).unwrap_or(spec.default.clone());
                let control = match (&spec.control, value) {
                    (KnobControl::Bool, KnobValue::Bool(on)) => {
                        let name = name.clone();
                        button(
                            ("styleguide-knob", index),
                            if on { "On" } else { "Off" },
                            on,
                        )
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
                                this.set(
                                    &name,
                                    KnobValue::Text(options[*choice].clone()),
                                    window,
                                    cx,
                                )
                            }),
                        )
                        .into_any_element()
                    }
                    (KnobControl::Text { .. }, KnobValue::Text(text)) => {
                        let editing = self.editing.as_deref() == Some(spec.name.as_str());
                        let knob = spec.name.clone();
                        div()
                            .id(("styleguide-knob", index))
                            .min_w(px(200.))
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .border_1()
                            .border_color(if editing {
                                palette.accent
                            } else {
                                palette.border
                            })
                            .child(if editing { format!("{text}▏") } else { text })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.editing = Some(knob.clone());
                                window.focus(&this.focus, cx);
                                cx.notify();
                            }))
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
                            .child(div().min_w(px(48.)).child(format!("{number}")))
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
                            .w(px(140.))
                            .flex_none()
                            .text_color(palette.muted)
                            .child(name),
                    )
                    .child(control)
            }))
    }
}

fn button(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    active: bool,
) -> gpui::Stateful<gpui::Div> {
    let palette = PALETTE();
    div()
        .id(id)
        .px_2()
        .py_0p5()
        .rounded_md()
        .border_1()
        .border_color(palette.border)
        .when(active, |this| {
            this.bg(palette.accent)
                .border_color(palette.accent)
                .text_color(palette.accent_text)
        })
        .when(!active, |this| this.hover(|this| this.bg(palette.sidebar)))
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

/// Opens the gallery in a new window through [`Styleguide::open_window`].
pub fn open(guide: Rc<Styleguide>, initial: Option<String>, cx: &mut App) -> anyhow::Result<()> {
    // The browser window is the page, whatever its requested bounds.
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(1200.), px(800.)),
            cx,
        ))),
        ..Default::default()
    };
    let open_window = guide.open_window.clone();
    open_window(
        options,
        Box::new(move |window, cx| {
            let gallery = cx.new(|cx| Gallery::new(guide, initial.as_deref(), window, cx));
            cx.new(|_| Frame::new(gallery.into()))
        }),
        cx,
    )?;
    Ok(())
}

/// Runs the gallery app: until its window closes natively; for the page's
/// lifetime on the web, where `initial` defaults to the URL's `#example-id`.
pub fn run(guide: Styleguide, initial: Option<String>) {
    let guide = Rc::new(guide);
    #[cfg(not(target_family = "wasm"))]
    let mut app = gpui_platform::application();
    #[cfg(all(feature = "web", target_family = "wasm"))]
    let mut app = crate::web::application();
    #[cfg(all(feature = "web", target_family = "wasm"))]
    let initial = initial.or_else(crate::web::linked_example);
    if let Some(assets) = guide.assets.clone() {
        app = app.with_assets(SharedAssets(assets));
    }
    app.run(move |cx| {
        guide.run_setup(cx);
        #[cfg(not(target_family = "wasm"))]
        cx.on_window_closed(|cx, _| cx.quit()).detach();
        if let Err(error) = open(guide, initial, cx) {
            crate::example::log_error(&format!("{error:#}"));
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
