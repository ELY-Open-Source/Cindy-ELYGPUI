//! A streaming assistant message: `MarkdownRenderer`'s two throttles, its chunks and their timer.

use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Duration};

use gpui::{App, Global, Task};

use super::SessionView;
use crate::{
    markdown::hast::Hast,
    motion::ReducedMotion,
    rich_answer, stream_fade,
    streamed_chunks::Chunks,
    streaming_throttle::Throttle,
    word_fade::{Fade, cache::States},
};

#[cfg(test)]
mod tests;

/// `MarkdownRendererBase` while it is mounted: its throttle, its fade state by key, its chunks.
struct Base {
    throttle: Throttle,
    fade: Option<(Option<String>, Rc<RefCell<Fade>>)>,
    chunks: Chunks,
}

/// A streaming message's renderer; `outer` is the throttle that looks for a rich answer.
struct Stream {
    outer: Throttle,
    base: Option<Base>,
    /// The time the timer fires at, with the timer.
    armed: Option<(f64, Task<()>)>,
}

/// The mounted content's streaming messages, by item key.
#[derive(Default)]
pub struct Streams(HashMap<String, Stream>);

/// `performance.timeOrigin`: where the clock's milliseconds count from.
struct Origin(std::time::Instant);

impl Global for Origin {}

/// The clock starts with the app, as a page's does with its load.
pub fn install(cx: &mut App) {
    cx.set_global(Origin(cx.background_executor().now()));
}

/// `performance.now()` on the executor clock.
fn now(cx: &App) -> f64 {
    let since = cx.background_executor().now() - cx.global::<Origin>().0;
    since.as_secs_f64() * 1000.0
}

fn streams(cx: &mut App) -> Option<&mut Streams> {
    let shown = cx.global_mut::<SessionView>().shown.as_mut()?;
    Some(&mut shown.content.as_mut()?.streams)
}

/// `streamFadeKey`: the task and the message's client id.
fn fade_key(client: Option<&str>, cx: &App) -> Option<String> {
    let task = cx.global::<SessionView>().shown.as_ref()?.id.as_str();
    Some(format!("{task}\0{}", client?))
}

/// The timer's callback: each throttle that waited emits, and the row draws again.
fn fire(key: &str, due: f64, cx: &mut App) {
    let now = now(cx).max(due);
    let Some(stream) = streams(cx).and_then(|streams| streams.0.get_mut(key)) else {
        return;
    };
    stream.armed = None;
    let inner = stream.base.as_mut().map(|base| &mut base.throttle);
    for throttle in [Some(&mut stream.outer), inner].into_iter().flatten() {
        if throttle.due().is_some_and(|at| at <= due) {
            throttle.fire(now);
        }
    }
    cx.refresh_windows();
}

/// The timer for the throttle that fires first, set anew when that time moved.
fn arm(stream: &mut Stream, key: &str, now: f64, cx: &mut App) {
    let inner = stream.base.as_ref().and_then(|base| base.throttle.due());
    let due = [stream.outer.due(), inner].into_iter().flatten();
    let Some(due) = due.reduce(f64::min) else {
        stream.armed = None;
        return;
    };
    if stream.armed.as_ref().is_some_and(|(at, _)| *at == due) {
        return;
    }
    let wait = Duration::from_secs_f64((due - now).max(0.0) / 1000.0);
    let key = key.to_owned();
    let timer = cx.spawn(async move |cx| {
        cx.background_executor().timer(wait).await;
        cx.update(|cx| fire(&key, due, cx));
    });
    stream.armed = Some((due, timer));
}

/// The streaming message's tree; none while its text holds a rich answer, which 9.10 draws.
pub(super) fn tree(
    key: &str,
    content: &str,
    client: Option<&str>,
    files: &[(String, String)],
    cx: &mut App,
) -> Option<Rc<Vec<Hast>>> {
    let now = now(cx);
    let fading = !cx.global::<ReducedMotion>().0 && stream_fade::enabled(cx);
    let fade_key = fade_key(client, cx);
    let held = streams(cx)?.0.remove(key);
    let mut stream = held.unwrap_or_else(|| Stream {
        outer: Throttle::new(content),
        base: None,
        armed: None,
    });
    stream.outer.render(content, true, now);
    let tree = match rich_answer::has(stream.outer.shown(content, true)) {
        true => {
            stream.base = None;
            None
        }
        false => {
            let base = stream.base.get_or_insert_with(|| Base {
                throttle: Throttle::new(content),
                fade: None,
                chunks: Chunks::default(),
            });
            base.throttle.render(content, true, now);
            // `useMemo` on the switch and the key: a kept state is asked for once.
            let kept = base
                .fade
                .take()
                .filter(|(key, _)| fading && *key == fade_key);
            let made = || {
                let states = cx.default_global::<States>();
                (fade_key.clone(), states.get(fade_key.as_deref()))
            };
            base.fade = fading.then(|| kept.unwrap_or_else(made));
            let text = base.throttle.shown(content, true);
            let fade = base.fade.as_ref().map(|(_, fade)| fade);
            Some(base.chunks.render(text, fade, files, now))
        }
    };
    arm(&mut stream, key, now, cx);
    streams(cx)?.0.insert(key.to_owned(), stream);
    tree
}

/// A message that streams no more: its renderer's state goes, and `releaseWordFadeState`.
pub(super) fn ended(key: &str, client: Option<&str>, cx: &mut App) {
    if let Some(streams) = streams(cx) {
        streams.0.remove(key);
    }
    if cx.default_global::<States>().is_empty() {
        return;
    }
    let fade_key = fade_key(client, cx);
    cx.global_mut::<States>().release(fade_key.as_deref());
}
