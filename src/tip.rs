//! Cindy's `Tip`: a dark card with 13px text, after gpui's 500ms delay (Cindy's too).

use gpui::{
    AnyView, App, AppContext, Context, IntoElement, ParentElement, Render, SharedString, Styled,
    Window, div, px,
};

use crate::{appearance_settings::text_px, theme::Appearance};

struct Tip(SharedString);

impl Render for Tip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let appearance = cx.global::<Appearance>();
        let size = text_px(13.0, cx);
        div()
            .max_w(px(420.0))
            .rounded(px(12.0))
            .px(px(10.0))
            .py(px(6.0))
            .text_size(size)
            .line_height(size * 1.375)
            .bg(appearance.token("tooltip-bg"))
            .text_color(appearance.token("tooltip-text"))
            .shadow(vec![appearance.shadow_token("shadow-menu")])
            .child(self.0.clone())
    }
}

/// A builder for gpui's `.tooltip()`.
pub fn tip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView {
    let text = text.into();
    move |_, cx| cx.new(|_| Tip(text.clone())).into()
}
