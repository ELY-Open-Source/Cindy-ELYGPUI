//! Cindy's `lib/toast.ts`: each window's toasts, at most three shown and the rest queued.

#![cfg_attr(
    not(test),
    expect(dead_code, reason = "the panel swap in 6.3d is the first caller")
)]

#[cfg(test)]
mod tests;
mod view;

use std::{
    collections::{HashMap, VecDeque},
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{App, BorrowAppContext, Global, SharedString, Task, Window, WindowId};

pub use view::ToastContainer;

const DEFAULT_DURATION: Duration = Duration::from_millis(1200);
const DEFAULT_ALERT_DURATION: Duration = Duration::from_millis(8000);
const MAX_ACTIVE: usize = 3;
const EXIT_ANIMATION: Duration = Duration::from_millis(300);
const MIN_RESUME_REMAINING: Duration = Duration::from_millis(1000);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    Loading,
    Info,
    Success,
    Warning,
    Error,
}

pub type OnClick = Rc<dyn Fn(&mut Window, &mut App)>;
pub type OnClose = Rc<dyn Fn(&mut App)>;

/// Who wrote a third party's message; drawn before it.
#[derive(Clone)]
pub struct Source {
    pub name: SharedString,
    pub icon: Option<Arc<gpui::Image>>,
}

/// One step tied to the message; the toast closes after it runs.
#[derive(Clone)]
pub struct Action {
    pub label: SharedString,
    pub on_click: OnClick,
}

/// `ToastOptions`; a zero duration stays until dismissed.
#[derive(Clone, Default)]
pub struct Options {
    pub duration: Option<Duration>,
    pub source: Option<Source>,
    pub action: Option<Action>,
    pub on_close: Option<OnClose>,
}

pub struct Item {
    pub id: u64,
    pub variant: Variant,
    pub message: SharedString,
    pub source: Option<Source>,
    pub action: Option<Action>,
    pub exiting: bool,
    duration: Duration,
    on_close: Option<OnClose>,
    timer: Option<Task<()>>,
    exit: Option<Task<()>>,
    expire_at: Option<Instant>,
    paused: Option<Duration>,
}

#[derive(Default)]
struct Board {
    active: Vec<Item>,
    queue: VecDeque<Item>,
}

/// Every window's board, as each Cindy renderer keeps its own store.
#[derive(Default)]
pub struct Toasts {
    boards: HashMap<WindowId, Board>,
    next_id: u64,
}

impl Global for Toasts {}

impl Toasts {
    /// The shown toasts, newest first.
    pub fn items(&self, window: WindowId) -> &[Item] {
        self.boards.get(&window).map_or(&[], |b| &b.active)
    }

    fn item(&mut self, window: WindowId, id: u64) -> Option<&mut Item> {
        let board = self.boards.get_mut(&window)?;
        board
            .active
            .iter_mut()
            .chain(board.queue.iter_mut())
            .find(|t| t.id == id)
    }
}

fn now(cx: &App) -> Instant {
    cx.background_executor().now()
}

fn start_timer(window: WindowId, item: &mut Item, duration: Duration, cx: &mut App) {
    if duration.is_zero() {
        return;
    }
    item.expire_at = Some(now(cx) + duration);
    let id = item.id;
    item.timer = Some(cx.spawn(async move |cx| {
        cx.background_executor().timer(duration).await;
        cx.update(|cx| dismiss(window, id, cx));
    }));
}

fn clear_timer(item: &mut Item) {
    item.timer = None;
    item.expire_at = None;
    item.paused = None;
}

fn create(
    window: &Window,
    variant: Variant,
    message: SharedString,
    options: Options,
    cx: &mut App,
) -> u64 {
    let window = window.window_handle().window_id();
    let alert = matches!(variant, Variant::Warning | Variant::Error);
    let default = if alert {
        DEFAULT_ALERT_DURATION
    } else {
        DEFAULT_DURATION
    };
    cx.update_global::<Toasts, _>(|toasts, cx| {
        toasts.next_id += 1;
        let mut item = Item {
            id: toasts.next_id,
            variant,
            message,
            source: options.source,
            action: options.action,
            exiting: false,
            duration: options.duration.unwrap_or(default),
            on_close: options.on_close,
            timer: None,
            exit: None,
            expire_at: None,
            paused: None,
        };
        let id = item.id;
        log::debug!(target: "toast", "{variant:?} toast {id}: {}", item.message);
        let board = toasts.boards.entry(window).or_default();
        if board.active.len() < MAX_ACTIVE {
            let duration = item.duration;
            start_timer(window, &mut item, duration, cx);
            board.active.insert(0, item);
        } else {
            board.queue.push_back(item);
        }
        id
    })
}

/// Neutral progress; the caller dismisses it when the work settles.
pub fn loading(
    window: &Window,
    message: impl Into<SharedString>,
    options: Options,
    cx: &mut App,
) -> u64 {
    let options = Options {
        duration: Some(options.duration.unwrap_or(Duration::ZERO)),
        ..options
    };
    create(window, Variant::Loading, message.into(), options, cx)
}

pub fn info(
    window: &Window,
    message: impl Into<SharedString>,
    options: Options,
    cx: &mut App,
) -> u64 {
    create(window, Variant::Info, message.into(), options, cx)
}

pub fn success(
    window: &Window,
    message: impl Into<SharedString>,
    options: Options,
    cx: &mut App,
) -> u64 {
    create(window, Variant::Success, message.into(), options, cx)
}

pub fn warning(
    window: &Window,
    message: impl Into<SharedString>,
    options: Options,
    cx: &mut App,
) -> u64 {
    create(window, Variant::Warning, message.into(), options, cx)
}

pub fn error(
    window: &Window,
    message: impl Into<SharedString>,
    options: Options,
    cx: &mut App,
) -> u64 {
    create(window, Variant::Error, message.into(), options, cx)
}

/// New text for a shown or queued toast; its clock keeps running.
pub fn update(window: WindowId, id: u64, message: impl Into<SharedString>, cx: &mut App) {
    cx.update_global::<Toasts, _>(|toasts, _| {
        if let Some(item) = toasts.item(window, id) {
            item.message = message.into();
        }
    });
}

/// Hover or focus holds the clock; at least a second is left on resume.
pub fn pause(window: WindowId, id: u64, cx: &mut App) {
    let at = now(cx);
    cx.update_global::<Toasts, _>(|toasts, _| {
        let Some(item) = toasts
            .item(window, id)
            .filter(|i| !i.exiting && i.timer.is_some())
        else {
            return;
        };
        item.timer = None;
        if let Some(expire) = item.expire_at.take() {
            item.paused = Some(
                expire
                    .saturating_duration_since(at)
                    .max(MIN_RESUME_REMAINING),
            );
        }
    });
}

pub fn resume(window: WindowId, id: u64, cx: &mut App) {
    cx.update_global::<Toasts, _>(|toasts, cx| {
        let Some(item) = toasts.item(window, id) else {
            return;
        };
        if let Some(remaining) = item.paused.take() {
            start_timer(window, item, remaining, cx);
        }
    });
}

/// `beginExit`: a shown toast fades for 300ms; a queued one goes at once.
pub fn dismiss(window: WindowId, id: u64, cx: &mut App) {
    let closed = cx.update_global::<Toasts, _>(|toasts, cx| {
        let board = toasts.boards.get_mut(&window)?;
        if let Some(index) = board.queue.iter().position(|t| t.id == id) {
            return board.queue.remove(index).and_then(|t| t.on_close);
        }
        let item = board.active.iter_mut().find(|t| t.id == id && !t.exiting)?;
        clear_timer(item);
        item.exiting = true;
        item.exit = Some(cx.spawn(async move |cx| {
            cx.background_executor().timer(EXIT_ANIMATION).await;
            cx.update(|cx| remove(window, id, cx));
        }));
        None
    });
    if let Some(on_close) = closed {
        on_close(cx);
    }
}

/// `finalizeRemove`: the faded toast goes and the queue fills its place.
fn remove(window: WindowId, id: u64, cx: &mut App) {
    let closed = cx.update_global::<Toasts, _>(|toasts, cx| {
        let board = toasts.boards.get_mut(&window)?;
        let index = board.active.iter().position(|t| t.id == id)?;
        let removed = board.active.remove(index);
        while board.active.len() < MAX_ACTIVE {
            let Some(mut next) = board.queue.pop_front() else {
                break;
            };
            let duration = next.duration;
            start_timer(window, &mut next, duration, cx);
            board.active.insert(0, next);
        }
        removed.on_close
    });
    if let Some(on_close) = closed {
        on_close(cx);
    }
}

pub fn dismiss_all(window: WindowId, cx: &mut App) {
    let (pending, shown) = cx.update_global::<Toasts, _>(|toasts, _| {
        let Some(board) = toasts.boards.get_mut(&window) else {
            return (Vec::new(), Vec::new());
        };
        let pending: Vec<_> = board.queue.drain(..).filter_map(|t| t.on_close).collect();
        (
            pending,
            board.active.iter().map(|t| t.id).collect::<Vec<_>>(),
        )
    });
    pending.into_iter().for_each(|on_close| on_close(cx));
    shown.into_iter().for_each(|id| dismiss(window, id, cx));
}

/// A closed window's board goes with it.
fn forget(window: WindowId, cx: &mut App) {
    cx.update_global::<Toasts, _>(|toasts, _| toasts.boards.remove(&window));
}
