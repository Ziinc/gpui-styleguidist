use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyView, App, Context, Hsla, IntoElement, ParentElement as _, Render, Styled as _, Window, div,
    px,
};
use std::rc::Rc;

/// Padding around a snapshot's content, in logical pixels.
pub(crate) const PADDING: f32 = 16.;

/// An example on the styleguide canvas: the theme background with padding, the
/// content at its intrinsic size.
pub(crate) struct Canvas {
    pub(crate) content: AnyView,
    pub(crate) background: Rc<dyn Fn(&App) -> Hsla>,
    pub(crate) foreground: Rc<dyn Fn(&App) -> Hsla>,
    pub(crate) fill: bool,
}

impl Render for Canvas {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg((self.background)(cx))
            .text_color((self.foreground)(cx))
            .p(px(PADDING))
            .flex()
            .items_start()
            .child(
                div()
                    .when(self.fill, |this| this.size_full())
                    .child(self.content.clone()),
            )
    }
}
