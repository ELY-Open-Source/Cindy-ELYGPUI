//! Cindy's `Tip`: a dark card with 13px text, after gpui's 500ms delay unless set.

use std::rc::Rc;

use gpui::{
    AnyElement, AnyView, App, AppContext, Context, InteractiveElement as _, IntoElement,
    ParentElement, Render, SharedString, Styled, Window, div,
};

use crate::units::css;
use crate::{appearance_settings::text_size, theme::Appearance};

type Build = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

struct Tip {
    max_width: f32,
    build: Build,
}

impl Render for Tip {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let appearance = cx.global::<Appearance>();
        let size = text_size(13.0, cx);
        div()
            .debug_selector(|| "tip".into())
            .max_w(css(self.max_width))
            .rounded(css(12.0))
            .px(css(10.0))
            .py(css(6.0))
            .text_size(css(size))
            .line_height(css(size * 1.375))
            .bg(appearance.token("tooltip-bg"))
            .text_color(appearance.token("tooltip-text"))
            .shadow(vec![appearance.shadow_token("shadow-menu", cx)])
            .child((self.build)(window, cx))
    }
}

/// A builder for gpui's `.tooltip()`.
pub fn tip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView {
    let text = text.into();
    rich(420.0, move |_, _| text.clone().into_any_element())
}

/// `Tip`'s `mono`: a path or a command in the code face.
pub fn mono(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView {
    use ely_gpui_component::theme::ActiveTheme as _;
    let text = text.into();
    rich(420.0, move |_, cx| {
        let family = cx.theme().mono_family.clone();
        div()
            .debug_selector(|| "tip-mono".into())
            .font_family(family)
            .child(text.clone())
            .into_any_element()
    })
}

/// A tip of other content, `max-w-[N]` wide.
pub fn rich(
    max_width: f32,
    build: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
) -> impl Fn(&mut Window, &mut App) -> AnyView {
    let build: Build = Rc::new(build);
    move |_, cx| {
        let build = build.clone();
        cx.new(|_| Tip { max_width, build }).into()
    }
}
