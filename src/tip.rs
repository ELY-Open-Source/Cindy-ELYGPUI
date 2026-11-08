//! Cindy's `Tip`: a dark card with 13px text, after gpui's 500ms delay (Cindy's too).

use gpui::{
    AnyView, App, AppContext, Context, IntoElement, ParentElement, Render, SharedString, Styled,
    Window, div,
};

use crate::units::css;
use crate::{appearance_settings::text_size, theme::Appearance};

struct Tip(SharedString);

impl Render for Tip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let appearance = cx.global::<Appearance>();
        let size = text_size(13.0, cx);
        div()
            .max_w(css(420.0))
            .rounded(css(12.0))
            .px(css(10.0))
            .py(css(6.0))
            .text_size(css(size))
            .line_height(css(size * 1.375))
            .bg(appearance.token("tooltip-bg"))
            .text_color(appearance.token("tooltip-text"))
            .shadow(vec![appearance.shadow_token("shadow-menu", cx)])
            .child(self.0.clone())
    }
}

/// A builder for gpui's `.tooltip()`.
pub fn tip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView {
    let text = text.into();
    move |_, cx| cx.new(|_| Tip(text.clone())).into()
}
