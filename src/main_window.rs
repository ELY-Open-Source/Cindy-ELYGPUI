use std::{path::PathBuf, time::Duration};

use anyhow::{Context as _, Result};
use ely_gpui_component::primitives::FocusScope;
use gpui::{
    App, AppContext, Bounds, Context, Entity, FocusHandle, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Render, Styled, Task, TitlebarOptions, Window, WindowBounds,
    WindowDecorations, WindowOptions, div, point, px, size,
};

use crate::{
    activation_click::{Input, Policy},
    app_shortcuts::AppShortcuts,
    chrome,
    layout_root::LayoutRoot,
    region,
    sidebar::Sidebar,
    theme::{Appearance, shell},
    toast::ToastContainer,
    units::{self, css},
    window_state::{self, WindowState},
};

const WINDOW_STATE_DELAY: Duration = Duration::from_millis(100);

/// The main window's root view.
pub struct Root {
    state: WindowState,
    user_data: PathBuf,
    pending_bounds: Option<Task<()>>,
    activation: Policy,
    first_press_held: bool,
    layout: Entity<LayoutRoot>,
    sidebar: Entity<Sidebar>,
    /// Held while nothing else is focused, so window-wide keys reach the root.
    focus: FocusHandle,
    /// Ely's Tab and Shift-Tab scope around the root.
    tab_scope: FocusHandle,
    toasts: Entity<ToastContainer>,
    actions: Entity<chrome::actions::ChromeActions>,
}

impl Root {
    /// `updateState`.
    fn record(&mut self, window: &Window, cx: &App) {
        let display = window.display(cx).map(|d| d.bounds());
        let (maximized, full_screen) = (window.is_maximized(), window.is_fullscreen());
        let normal = normal_bounds(window.window_bounds(), maximized, full_screen);
        self.state.update(normal, maximized, full_screen, display);
    }

    /// The `close` event's save, for system and drawn closes alike.
    fn save_state(&mut self, window: &Window, cx: &App) {
        self.record(window, cx);
        if let Err(error) = self.state.save(&self.user_data) {
            log::warn!(target: "window-state", "{error:#}");
        }
    }

    fn press(&mut self, input: Input, first_mouse: bool, cx: &mut Context<Self>) {
        if self
            .activation
            .swallow(input, first_mouse, &mut self.first_press_held)
        {
            log::debug!(target: "window", "swallowed activation {input:?}");
            cx.stop_propagation();
        }
    }
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let controls = (!cfg!(target_os = "macos")).then(|| {
            let close = cx.listener(|root, (), window, cx| {
                log::info!(target: "window", "close requested");
                root.save_state(window, cx);
                window.remove_window();
            });
            div()
                .absolute()
                .right_0()
                .top_0()
                .h(css(chrome::TOP_ROW))
                .flex()
                .items_center()
                .pr_2()
                .child(chrome::window_controls(close, cx))
        });
        let edges = chrome::resize_edges(window, cx);
        let wallpaper = crate::wallpaper::canvas(window, cx);
        let appearance = cx.global::<Appearance>();
        let root = div()
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .flex()
            .bg(crate::window_material::root(cx))
            .text_color(appearance.token("text-primary"))
            .capture_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                if cx.global::<AppShortcuts>().matches("toggle-sidebar", event) {
                    this.sidebar.update(cx, |sidebar, cx| sidebar.toggle(cx));
                    cx.stop_propagation();
                }
                for (id, direction) in
                    [("session-history-back", -1), ("session-history-forward", 1)]
                {
                    if cx.global::<AppShortcuts>().matches(id, event)
                        && crate::browse_history::go(direction, cx)
                    {
                        cx.stop_propagation();
                    }
                }
            }))
            .capture_any_mouse_down(cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                if event.button == MouseButton::Left {
                    this.press(Input::Down, event.first_mouse, cx);
                }
            }))
            .capture_any_mouse_up(cx.listener(|this, event: &gpui::MouseUpEvent, _, cx| {
                if event.button == MouseButton::Left {
                    this.press(Input::Up, false, cx);
                }
            }))
            .children(wallpaper)
            .child(self.sidebar.clone())
            .child(self.layout.clone())
            .children(controls)
            .children(edges)
            .child(self.actions.clone())
            .child(crate::sessions::interrupted::dialog::view(window, cx))
            .children(crate::confirm::view(window, cx))
            .child(self.toasts.clone())
            .children(crate::localdb::screen(cx));
        FocusScope::new(&self.tab_scope).size_full().child(root)
    }
}

/// The root's key context: gpui's `!` predicates need one on every focus path.
pub const CONTEXT: &str = "CindyWindow";

/// `isNormal`: gpui's windowed placement, which stays normal while minimized on Windows;
/// macOS reports `Windowed` while maximized, so the flags must agree too.
fn normal_bounds(
    bounds: WindowBounds,
    maximized: bool,
    full_screen: bool,
) -> Option<Bounds<gpui::Pixels>> {
    match bounds {
        WindowBounds::Windowed(bounds) if !maximized && !full_screen => Some(bounds),
        _ => None,
    }
}

/// `swallowActivationClick` from `window-behavior-settings.json`, as its store reads it.
pub fn mac_swallow_setting(user_data: &std::path::Path) -> bool {
    match crate::cindy_json::read(&user_data.join("window-behavior-settings.json"), None) {
        Ok(fields) => fields
            .and_then(|f| {
                crate::cindy_json::boolean(f.get("swallowActivationClick").map(AsRef::as_ref))
            })
            .unwrap_or(false),
        Err(error) => {
            log::warn!(target: "window-behavior-settings-store", "window-behavior settings read failed; falling back to defaults: {error:#}");
            false
        }
    }
}

/// The settings that decide whether the activating click is swallowed.
pub struct Activation {
    pub mac_setting: bool,
    pub windows_setting: bool,
}

/// Opens the main window as Cindy's `createWindow` does: identity, restored state, minimum size.
pub fn open(user_data: PathBuf, activation: Activation, cx: &mut App) -> Result<()> {
    let raw = window_state::read(&user_data);
    let displays: Vec<Bounds<gpui::Pixels>> = cx.displays().iter().map(|d| d.bounds()).collect();
    let primary = cx
        .primary_display()
        .map(|d| d.bounds())
        .or(displays.first().copied());
    let state = window_state::restore(raw.as_ref(), &displays, primary);
    let restore_mac_fullscreen = cfg!(target_os = "macos") && state.is_full_screen;
    let restore_maximized = !restore_mac_fullscreen
        && (window_state::persisted_maximized(raw.as_ref()) || state.is_maximized);
    let window_size = size(px(state.width as f32), px(state.height as f32));
    let bounds = match (state.x, state.y) {
        (Some(x), Some(y)) => Bounds {
            origin: point(px(x as f32), px(y as f32)),
            size: window_size,
        },
        _ => Bounds::centered(None, window_size, cx),
    };
    let window_bounds = if !cfg!(target_os = "macos") && state.is_full_screen {
        WindowBounds::Fullscreen(bounds)
    } else if restore_maximized {
        WindowBounds::Maximized(bounds)
    } else {
        WindowBounds::Windowed(bounds)
    };
    log::info!(target: "window", "restoring {window_bounds:?} from {state:?}");
    let options = WindowOptions {
        window_bounds: Some(window_bounds),
        titlebar: Some(TitlebarOptions {
            title: Some("Cindy".into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(12.0), px(16.0))),
        }),
        window_decorations: Some(WindowDecorations::Client),
        app_id: Some(region::APP_ID.to_owned()),
        window_min_size: Some(size(px(800.0), px(600.0))),
        ..Default::default()
    };
    let policy = Policy::new(activation.mac_setting, activation.windows_setting);
    cx.open_window(options, |window, cx| {
        window.set_rem_size(units::rem_size(cx));
        crate::window_material::apply(window, cx);
        window
            .observe_window_visibility(|visibility, window, cx| {
                log::info!(target: "main-window", "window {visibility:?}");
                crate::wallpaper::visibility_changed(window, cx)
            })
            .detach();
        cx.new(|cx| {
            cx.observe_window_appearance(window, shell::follow_os)
                .detach();
            cx.observe_window_activation(window, |root: &mut Root, window, cx| {
                let active = window.is_window_active();
                if active {
                    crate::motion::refresh_reduced_motion(cx);
                    crate::workspace_media::focused(cx);
                    crate::session_view::images_focused(cx);
                }
                root.activation.activation(active);
                if !active {
                    root.sidebar.update(cx, |sidebar, cx| sidebar.blur(cx));
                }
            })
            .detach();
            cx.observe_window_bounds(window, |root: &mut Root, window, cx| {
                root.pending_bounds = Some(cx.spawn_in(window, async move |root, cx| {
                    cx.background_executor().timer(WINDOW_STATE_DELAY).await;
                    let recorded = root.update_in(cx, |root, window, cx| root.record(window, cx));
                    if let Err(error) = recorded {
                        log::debug!(target: "window", "window gone before bounds settled: {error}");
                    }
                }));
            })
            .detach();
            let root = cx.entity().downgrade();
            window.on_window_should_close(cx, move |window, cx| {
                log::info!(target: "window", "close requested");
                if let Err(error) = root.update(cx, |root, cx| root.save_state(window, cx)) {
                    log::warn!(target: "window-state", "root gone at close: {error}");
                }
                true
            });
            if restore_mac_fullscreen {
                window.on_next_frame(|window, _| window.toggle_fullscreen());
            }
            log::info!(target: "window", "decorations {:?}", window.window_decorations());
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            cx.on_focus_lost(window, |root: &mut Root, window, cx| {
                log::debug!(target: "window", "focus lost; the root takes it");
                window.focus(&root.focus, cx);
            })
            .detach();
            let sidebar = cx.new(Sidebar::new);
            let actions = cx.new(|cx| chrome::actions::ChromeActions::new(sidebar.clone(), cx));
            let painted = sidebar.downgrade();
            let layout = cx.new(|cx| {
                let seen = move |cx: &App| {
                    let sidebar = painted
                        .upgrade()
                        .expect("the sidebar lives with the window");
                    sidebar.read(cx).seen(cx)
                };
                LayoutRoot::new(Box::new(seen), cx)
            });
            cx.observe(&sidebar, {
                let layout = layout.downgrade();
                move |_, _, cx| {
                    if let Some(layout) = layout.upgrade() {
                        layout.update(cx, |_, cx| cx.notify());
                    }
                }
            })
            .detach();
            Root {
                state,
                user_data,
                pending_bounds: None,
                activation: policy,
                first_press_held: false,
                layout,
                sidebar,
                focus,
                tab_scope: cx.focus_handle(),
                toasts: cx.new(|cx| ToastContainer::new(window, cx)),
                actions,
            }
        })
    })
    .context("window failed to open")?;
    log::info!(target: "window", "main window opened as {}", region::APP_ID);
    Ok(())
}

#[cfg(test)]
mod history_tests;
#[cfg(test)]
mod screen_tests;

#[cfg(test)]
mod tests {
    #[test]
    fn only_a_truly_normal_window_gives_bounds() {
        use gpui::{Bounds, WindowBounds, point, px, size};
        let b = Bounds {
            origin: point(px(10.0), px(20.0)),
            size: size(px(900.0), px(700.0)),
        };
        assert_eq!(
            super::normal_bounds(WindowBounds::Windowed(b), false, false),
            Some(b)
        );
        assert_eq!(
            super::normal_bounds(WindowBounds::Windowed(b), true, false),
            None,
            "macOS maximized"
        );
        assert_eq!(
            super::normal_bounds(WindowBounds::Windowed(b), false, true),
            None
        );
        assert_eq!(
            super::normal_bounds(WindowBounds::Maximized(b), true, false),
            None
        );
    }

    #[test]
    fn swallow_setting_survives_an_overflowing_sibling() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("window-behavior-settings.json");
        assert!(!super::mac_swallow_setting(dir.path()));
        std::fs::write(
            &file,
            r#"{"swallowActivationClick":true,"awayGuardDelaySeconds":1e400}"#,
        )
        .unwrap();
        assert!(super::mac_swallow_setting(dir.path()));
        std::fs::write(&file, "[]").unwrap();
        assert!(!super::mac_swallow_setting(dir.path()));
    }
}
