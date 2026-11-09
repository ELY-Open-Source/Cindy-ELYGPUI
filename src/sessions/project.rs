//! `ProjectNode`'s `ProjectHeader`: folds, renames, and opens the project menu.

mod alias;
mod remove;
mod rename;

use std::rc::Rc;

use ely_gpui_component::{
    menus::{ContextMenu, Menu, MenuItem},
    primitives::IconName,
};
use gpui::{
    App, ClickEvent, ClipboardItem, ElementId, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement as _, Pixels, Point, RenderOnce, Role,
    StatefulInteractiveElement as _, Styled as _, Window, div, point, prelude::FluentBuilder as _,
};

use super::{
    group::Project,
    keyboard::{self, Probe},
    main_list::{Name, Toggle, header},
    paths::encode_component,
    pin, search,
};
use crate::{
    i18n::Translator,
    lucide::icon,
    theme::Appearance,
    tip::tip,
    toast,
    units::{css, window_px},
};

pub use rename::Editing;

/// `buildProjectDeepLink`: `strictEncodeURIComponent` also escapes `!'()*`.
pub fn deep_link(dir: &str) -> String {
    let mut out = String::from("cindy://project/");
    for ch in encode_component(dir).chars() {
        match ch {
            '!' | '\'' | '(' | ')' | '*' => out.push_str(&format!("%{:02X}", ch as u8)),
            _ => out.push(ch),
        }
    }
    out
}

/// `handleCopyDeepLink`.
fn copy_link(dir: &str, window: &mut Window, cx: &mut App) {
    cx.write_to_clipboard(ClipboardItem::new_string(deep_link(dir)));
    log::info!(target: "sessions", "copied the link of project {dir}");
    let message = cx
        .global::<Translator>()
        .t("ccAgent.sidebar.deepLink.copied", &[]);
    toast::success(window, message, toast::Options::default(), cx);
}

/// `handleOpenInExplorer`: the handler's error, else `openFolderFailed`, as a toast.
fn open_folder(dir: &str, window: &mut Window, cx: &mut App) {
    let opening = crate::open_path::open(dir, cx);
    let handle = window.window_handle();
    cx.spawn(async move |cx| {
        let Err(error) = opening.await else {
            return;
        };
        cx.update(|cx| {
            let message = match error.is_empty() {
                true => cx
                    .global::<Translator>()
                    .t("ccAgent.common.openFolderFailed", &[]),
                false => error,
            };
            let shown = handle.update(cx, |_, window, cx| {
                toast::error(window, message, toast::Options::default(), cx);
            });
            if shown.is_err() {
                log::warn!(target: "sessions", "open folder: its window closed");
            }
        });
    })
    .detach();
}

/// Show files, Import and Archive all come with their tasks.
fn menu(p: &Project, cx: &App) -> Menu {
    let t = cx.global::<Translator>();
    let label = |key: &str| t.t(&format!("ccAgent.sidebar.projectAction.{key}"), &[]);
    let pinned = pin::project_pinned(&p.key, cx);
    let (key, name, dir) = (p.key.clone(), p.name.clone(), p.dir.clone());
    let folder = p.dir.clone();
    let pin_key = p.key.clone();
    let (remove_key, remove_name) = (p.key.clone(), p.name.clone());
    let local = p.key.starts_with("local:");
    let remote = p.key.starts_with("remote:");
    let scope = search::lock::Lock::Project {
        key: p.key.clone(),
        name: p.name.clone(),
        ids: p.sessions.iter().map(|s| s.id.clone()).collect(),
    };
    let menu = Menu::new()
        .item(
            MenuItem::new(label("rename"))
                .on_click(move |window, cx| rename::start(&key, &name, window, cx)),
        )
        .item(
            MenuItem::new(label(if pinned { "unpin" } else { "pin" }))
                .on_click(move |window, cx| pin::project(&pin_key, pinned, window, cx)),
        )
        .separator()
        .item(
            MenuItem::new(label("search"))
                .on_click(move |window, cx| search::lock::request(scope.clone(), window, cx)),
        );
    // A remote project's folder is not on this computer.
    let menu = match remote {
        true => menu,
        false => menu
            .item(
                MenuItem::new(label("openInExplorer"))
                    .on_click(move |window, cx| open_folder(&folder, window, cx)),
            )
            .separator()
            .item(
                MenuItem::new(label("copyDeepLink"))
                    .on_click(move |window, cx| copy_link(&dir, window, cx)),
            ),
    }
    .separator();
    if !local {
        return menu;
    }
    menu.item(
        MenuItem::new(label("removeFromSidebar"))
            .on_click(move |window, cx| remove::remove(&remove_key, &remove_name, window, cx)),
    )
}

/// `ProjectNode`'s header for `project`.
#[derive(IntoElement)]
pub struct Head {
    project: Project,
    collapsed: bool,
    toggle: Toggle,
}

impl Head {
    pub fn new(project: &Project, collapsed: bool, toggle: Toggle) -> Self {
        Head {
            project: project.clone(),
            collapsed,
            toggle,
        }
    }
}

type Asked = Entity<Option<(u64, Point<Pixels>)>>;

/// The keys on the header: Enter and Space fold it, the menu key opens the menu.
struct Keys {
    key: String,
    focus: FocusHandle,
    head: Probe,
    more: Probe,
    asked: Asked,
    toggle: Toggle,
}

impl Keys {
    fn pressed(&self, k: &gpui::Keystroke, up: bool, window: &mut Window, cx: &mut App) {
        if rename::editing(&self.key, cx).is_some() {
            return;
        }
        let own = self.focus.is_focused(window);
        if !up && own && matches!(k.key.as_str(), "enter" | "space") {
            window.prevent_default();
            (self.toggle)(cx);
            return;
        }
        if !keyboard::menu_key(k, up) {
            return;
        }
        cx.stop_propagation();
        let probe = if own { &self.head } else { &self.more };
        let Some(bounds) = probe.get() else {
            return log::warn!(target: "sessions", "project {} menu key: no bounds drawn", self.key);
        };
        log::info!(target: "sessions", "project {} menu from the keyboard", self.key);
        keyboard::ask(&self.asked, bounds.center(), cx);
    }
}

impl RenderOnce for Head {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = &self.project;
        let id = format!("project:{}-header", p.key);
        let part = |part: &str| ElementId::Name(format!("project-{part}-{}", p.key).into());
        let tab_stop = |window: &mut Window, cx: &mut App, part| {
            window
                .use_keyed_state(part, cx, |_, cx| cx.focus_handle().tab_stop(true))
                .read(cx)
                .clone()
        };
        let focus = tab_stop(window, cx, part("focus"));
        let more_focus = tab_stop(window, cx, part("more-focus"));
        let asked: Asked = window.use_keyed_state(part("menu-asked"), cx, |_, _| None);
        let hovered = window.use_keyed_state(part("hover"), cx, |_, _| false);
        let lit = *hovered.read(cx);
        let request = *asked.read(cx);
        let glyph = match self.collapsed {
            true => IconName::Folder,
            false => IconName::FolderOpen,
        };
        let field = rename::editing(&p.key, cx).map(|e| rename::field(e, cx));
        let editing = field.is_some();
        let name = field.map_or(Name::Text(p.name.clone()), Name::Field);
        let keys = Rc::new(Keys {
            key: p.key.clone(),
            focus: focus.clone(),
            head: Probe::default(),
            more: Probe::default(),
            asked: asked.clone(),
            toggle: self.toggle.clone(),
        });
        let (down, up, right) = (keys.clone(), keys.clone(), asked.clone());
        let (toggle, key, display) = (self.toggle.clone(), p.key.clone(), p.name.clone());
        let more_at = keys.more.clone();
        let head = header(
            id.clone().into(),
            glyph.svg(),
            (name, Some(lit)),
            self.collapsed,
            cx,
        )
        .hover_listener_mode(gpui::HoverListenerMode::InputModalityIndependent)
        .on_hover(move |h, _, cx| {
            hovered.update(cx, |hovered, cx| {
                *hovered = *h;
                cx.notify();
            })
        })
        .track_focus(&focus)
        .role(Role::Button)
        .aria_expanded(!self.collapsed)
        .on_click(move |event, window, cx| {
            let ClickEvent::Mouse(m) = event else {
                return;
            };
            if rename::editing(&key, cx).is_some() {
                return;
            }
            toggle(cx);
            if m.up.click_count == 2 {
                rename::start(&key, &display, window, cx);
            }
        })
        .on_key_down(move |e, window, cx| down.pressed(&e.keystroke, false, window, cx))
        .on_key_up(move |e, window, cx| up.pressed(&e.keystroke, true, window, cx))
        .on_mouse_down(MouseButton::Right, move |e, window, cx| {
            window.prevent_default();
            cx.stop_propagation();
            keyboard::ask(&right, e.position, cx);
        })
        .child(keyboard::probe(keys.head.clone()))
        .when(!editing, |el| {
            el.child(slot(
                (format!("project-slot-{}", p.key), lit),
                More {
                    id: p.key.clone(),
                    focus: more_focus,
                    probe: more_at,
                    asked,
                },
                window,
            ))
        });
        ContextMenu::new(part("menu"), menu(p, cx))
            .manual(request)
            .w_full()
            .child(head)
    }
}

/// The right slot: a hover or a focused More widens it and shows More.
fn slot((selector, lit): (String, bool), more: More, window: &Window) -> impl IntoElement {
    let shown = lit || more.focus.is_focused(window);
    div()
        .debug_selector(move || selector)
        .relative()
        .flex()
        .flex_none()
        .h(css(24.0))
        .ml_auto()
        .items_center()
        .justify_end()
        .child(div().hidden().size(css(20.0)).when(shown, |el| el.flex()))
        .child(
            div()
                .absolute()
                .top_0()
                .right_0()
                .flex()
                .h(css(24.0))
                .items_center()
                .opacity(if shown { 1.0 } else { 0.0 })
                .child(more),
        )
}

/// `ProjectAction`'s More; the menu opens 2px under its left edge.
#[derive(IntoElement)]
struct More {
    id: String,
    focus: FocusHandle,
    probe: Probe,
    asked: Asked,
}

impl RenderOnce for More {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let key = ElementId::Name(format!("project-more-hover-{}", self.id).into());
        let hovered = window.use_keyed_state(key, cx, |_, _| false);
        let lit = *hovered.read(cx);
        let appearance = cx.global::<Appearance>();
        let (color, hover_bg, hover_fg) = (
            appearance.hsl_token("sidebar-action-icon"),
            appearance.hsl_token("sidebar-item-hover"),
            appearance.hsl_token("foreground"),
        );
        let label = cx
            .global::<Translator>()
            .t("ccAgent.common.moreActions", &[]);
        let selector = format!("project-more-{}", self.id);
        let lit_selector = format!("project-more-hover-{}", self.id);
        let (probe, asked) = (self.probe.clone(), self.asked);
        div()
            .id(ElementId::Name(selector.clone().into()))
            .debug_selector(move || selector)
            .track_focus(&self.focus)
            .role(Role::Button)
            .aria_label(label.clone())
            .relative()
            .flex()
            .flex_none()
            .size(css(20.0))
            .items_center()
            .justify_center()
            .rounded(css(6.0))
            .hover_listener_mode(gpui::HoverListenerMode::InputModalityIndependent)
            .on_hover(move |h, _, cx| {
                hovered.update(cx, |hovered, cx| {
                    *hovered = *h;
                    cx.notify();
                })
            })
            .tooltip(tip(label))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            // The menu key's `contextmenu` still reaches the header.
            .on_key_down(|e, _, cx| {
                if !keyboard::menu_key(&e.keystroke, false) {
                    cx.stop_propagation();
                }
            })
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                let Some(b) = probe.get() else {
                    return log::warn!(target: "sessions", "project More: no bounds drawn");
                };
                let at = point(b.origin.x, b.origin.y + b.size.height + window_px(2.0, cx));
                keyboard::ask(&asked, at, cx);
            })
            .when(lit, |el| {
                el.child(
                    div()
                        .debug_selector(move || lit_selector)
                        .absolute()
                        .inset_0()
                        .rounded(css(6.0))
                        .bg(hover_bg),
                )
            })
            .child(
                icon(IconName::EllipsisVertical.svg(), 14.0, 2.0)
                    .relative()
                    .text_color(if lit { hover_fg } else { color }),
            )
            .child(keyboard::probe(self.probe))
    }
}

#[cfg(test)]
mod tests;
