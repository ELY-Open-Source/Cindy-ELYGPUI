//! `Toast.tsx` and `ToastContainer.tsx`.

use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Duration, time::Instant};

use ely_gpui_component::{
    buttons::{Button, ButtonVariant},
    motion::Spinner,
    primitives::{Icon, IconName},
    theme::{ControlSize, IconSize},
};
use gpui::{
    Animation, AnimationExt, AnyElement, App, Bounds, Context, Element, ElementId, FocusHandle,
    GlobalElementId, InspectorElementId, InteractiveElement, IntoElement, LayoutId, ParentElement,
    Pixels, Render, Role, StatefulInteractiveElement, Styled, Window, WindowId, canvas, div, img,
    point, prelude::FluentBuilder, px, rgb,
};

use super::{Item, Toasts, Variant};
use crate::units::{self, css};
use crate::{
    appearance_settings::text_size, i18n::Translator, motion::ease_out, theme::Appearance,
};

const FADE: Duration = Duration::from_millis(300);

/// `VARIANT_MAP`: icon, color and role; loading follows the theme's secondary text.
fn meta(variant: Variant, appearance: &Appearance) -> (Option<IconName>, gpui::Hsla, Role) {
    match variant {
        Variant::Loading => (None, appearance.token("text-secondary"), Role::Status),
        Variant::Info => (Some(IconName::Info), rgb(0x417CDD).into(), Role::Status),
        Variant::Success => (
            Some(IconName::CircleCheck),
            rgb(0x2AAE5B).into(),
            Role::Status,
        ),
        Variant::Warning => (
            Some(IconName::TriangleAlert),
            rgb(0xF3A115).into(),
            Role::Status,
        ),
        Variant::Error => (Some(IconName::CircleX), rgb(0xD91F37).into(), Role::Alert),
    }
}

/// What one frame draws of a toast.
struct Card {
    id: u64,
    variant: Variant,
    message: gpui::SharedString,
    source: Option<super::Source>,
    action: Option<super::Action>,
    exiting: bool,
}

impl Card {
    fn of(item: &Item) -> Self {
        Card {
            id: item.id,
            variant: item.variant,
            message: item.message.clone(),
            source: item.source.clone(),
            action: item.action.clone(),
            exiting: item.exiting,
        }
    }
}

/// Per toast: wrapped text or action, hover and focus, and its focus handle.
struct Held {
    multiline: bool,
    hovering: bool,
    focused: bool,
    focus: FocusHandle,
}

/// The natural top each toast had, and its slide from there.
#[derive(Clone, Copy)]
struct Track {
    top: Pixels,
    from: Pixels,
    start: Instant,
}

type Tracks = Rc<RefCell<HashMap<u64, Track>>>;

pub struct ToastContainer {
    window: WindowId,
    held: HashMap<u64, Held>,
    tracks: Tracks,
}

impl ToastContainer {
    pub fn new(window: &Window, cx: &mut Context<Self>) -> Self {
        let window = window.window_handle().window_id();
        cx.observe_global::<Toasts>(|_, cx| cx.notify()).detach();
        cx.on_release(move |_, cx| super::forget(window, cx))
            .detach();
        ToastContainer {
            window,
            held: HashMap::new(),
            tracks: Rc::default(),
        }
    }

    #[cfg(test)]
    pub fn multiline(&self) -> Vec<(u64, bool)> {
        let mut out: Vec<_> = self.held.iter().map(|(id, h)| (*id, h.multiline)).collect();
        out.sort();
        out
    }

    /// Hover and keyboard focus each hold the clock; it runs again once both leave.
    fn hold(&mut self, id: u64, hovering: Option<bool>, focused: Option<bool>, cx: &mut App) {
        let Some(held) = self.held.get_mut(&id) else {
            return;
        };
        let was = held.hovering || held.focused;
        held.hovering = hovering.unwrap_or(held.hovering);
        held.focused = focused.unwrap_or(held.focused);
        match (was, held.hovering || held.focused) {
            (false, true) => super::pause(self.window, id, cx),
            (true, false) => super::resume(self.window, id, cx),
            _ => {}
        }
    }

    fn toast(&self, item: &Card, cx: &mut Context<Self>) -> AnyElement {
        let appearance = cx.global::<Appearance>();
        let (icon, color, role) = meta(item.variant, appearance);
        let held = &self.held[&item.id];
        let size = text_size(13.0, cx);
        let line = size * 1.375;
        let id = item.id;
        let entity = cx.entity();
        let measure = canvas(
            move |bounds: Bounds<Pixels>, _, cx| {
                let multiline = bounds.size.height > units::window_px(line + 1.0, cx);
                entity.update(cx, |this, cx| {
                    if let Some(held) = this.held.get_mut(&id).filter(|h| h.multiline != multiline)
                    {
                        held.multiline = multiline;
                        cx.notify();
                    }
                });
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();
        let source = item.source.as_ref().map(|source| {
            let tertiary = appearance.token("text-tertiary");
            div()
                .flex_none()
                .mr_2()
                .text_size(css(size))
                .line_height(css(line))
                .font_weight(gpui::FontWeight::MEDIUM)
                .flex()
                .items_center()
                .gap(css(6.0))
                .when_some(source.icon.clone(), |el, icon| {
                    el.child(img(icon).size_4().rounded(css(4.0)))
                })
                .child(
                    div()
                        .max_w(css(160.0))
                        .truncate()
                        .text_color(tertiary)
                        .child(source.name.clone()),
                )
                .child(div().text_color(tertiary).child("·"))
        });
        let message = div()
            .relative()
            .min_w_0()
            .max_w(css(480.0))
            .text_size(css(size))
            .line_height(css(line))
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(appearance.token("cmd-palette-item-text"))
            .child(measure)
            .child(item.message.clone());
        let action = item.action.clone().map(|action| {
            Button::new(("toast-action", id as usize), action.label.clone())
                .variant(ButtonVariant::Secondary)
                .size(ControlSize::Sm)
                .focus_handle(&held.focus)
                .on_click(move |_, window, cx| {
                    (action.on_click)(window, cx);
                    super::dismiss(window_id(window), id, cx);
                })
        });
        let indicator = match icon {
            Some(icon) => Icon::new(icon)
                .size(IconSize::Md)
                .color(color)
                .into_any_element(),
            None => Spinner::new(("toast-spinner", id as usize))
                .size(IconSize::Md)
                .color(color)
                .into_any_element(),
        };
        let radius = if held.multiline {
            css(12.0)
        } else {
            css(9999.0)
        };
        div()
            .id(("toast", id as usize))
            .role(role)
            .track_focus(&held.focus)
            .max_w_full()
            .flex()
            .items_center()
            .gap_2()
            .rounded(radius)
            .border(css(1.0))
            .border_color(appearance.token("cmd-palette-border"))
            .bg(crate::window_material::overlay(cx))
            .px_4()
            .py(css(10.0))
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                this.hold(id, Some(*hovered), None, cx)
            }))
            .child(indicator)
            .children(source)
            .child(message)
            .children(action.map(|a| div().ml_1().max_w(gpui::relative(0.4)).child(a)))
            .into_any_element()
    }
}

fn window_id(window: &Window) -> WindowId {
    window.window_handle().window_id()
}

impl Render for ToastContainer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let toasts = cx.global::<Toasts>().items(self.window);
        let ids: Vec<(u64, bool)> = toasts.iter().map(|t| (t.id, t.exiting)).collect();
        self.held.retain(|id, _| ids.iter().any(|(i, _)| i == id));
        self.tracks
            .borrow_mut()
            .retain(|id, _| ids.iter().any(|(i, _)| i == id));
        for (id, _) in &ids {
            self.held.entry(*id).or_insert_with(|| Held {
                multiline: false,
                hovering: false,
                focused: false,
                focus: cx.focus_handle(),
            });
        }
        let focus: Vec<(u64, bool)> = self
            .held
            .iter()
            .map(|(id, h)| (*id, h.focus.contains_focused(window, cx)))
            .collect();
        for (id, focused) in focus {
            self.hold(id, None, Some(focused), cx);
        }
        let cards: Vec<Card> = cx
            .global::<Toasts>()
            .items(self.window)
            .iter()
            .map(Card::of)
            .collect();
        let cards: Vec<_> = cards
            .iter()
            .map(|card| (card.id, card.exiting, self.toast(card, cx)))
            .collect();
        let viewport = window.viewport_size().width;
        let label = cx
            .global::<Translator>()
            .t("commonUi.toastContainer.ariaLabel", &[]);
        // `z-[10100]`: over every dialog.
        let container = div()
            .id("toast-container")
            .aria_label(label)
            .absolute()
            .left_0()
            .right_0()
            .top(css(20.0))
            .flex()
            .flex_col()
            .items_center()
            .gap(css(10.0))
            .children(cards.into_iter().map(|(id, exiting, card)| {
                let fade = Animation::new(FADE).with_easing(ease_out);
                let faded = div().child(card);
                let faded = if exiting {
                    faded
                        .with_animation(("toast-out", id as usize), fade, |el, d| {
                            el.opacity(1.0 - d)
                        })
                        .into_any_element()
                } else {
                    faded
                        .with_animation(("toast-in", id as usize), fade, |el, d| el.opacity(d))
                        .into_any_element()
                };
                let slide = Slide {
                    id,
                    exiting,
                    tracks: self.tracks.clone(),
                    child: faded,
                };
                div()
                    .max_w(viewport - units::window_px(32.0, cx))
                    .child(slide)
            }));
        gpui::deferred(container).with_priority(crate::confirm_dialog::PRIORITY + 1)
    }
}

/// `ToastContainer`'s FLIP: a toast that moves slides from its old top over 300ms.
struct Slide {
    id: u64,
    exiting: bool,
    tracks: Tracks,
    child: AnyElement,
}

impl IntoElement for Slide {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Slide {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let now = cx.background_executor().now();
        let top = bounds.origin.y;
        let offset = {
            let mut tracks = self.tracks.borrow_mut();
            let track = tracks.entry(self.id).or_insert(Track {
                top,
                from: px(0.0),
                start: now,
            });
            if !self.exiting && (track.top - top).abs() >= px(1.0) {
                track.from = track.top - top;
                track.start = now;
            }
            track.top = top;
            let t = now.saturating_duration_since(track.start).as_secs_f32() / FADE.as_secs_f32();
            if t < 1.0 && track.from != px(0.0) {
                window.request_animation_frame();
            }
            if self.exiting {
                px(0.0)
            } else {
                track.from * (1.0 - ease_out(t))
            }
        };
        window.with_element_offset(point(px(0.0), offset), |window| {
            self.child.prepaint(window, cx)
        });
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}
