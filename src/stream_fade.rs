//! `useStreamFadePreference.ts`: whether a streamed answer's words fade in.
#![cfg_attr(
    not(test),
    expect(dead_code, reason = "a streamed row reads it in 9.6j3b")
)]

use std::cell::Cell;

use gpui::App;

use crate::preferences::Preferences;

const KEY: &str = "chat.streamFadePreference";

/// `memoryValue`: the choice once read, on or off.
#[derive(Default)]
pub struct StreamFade(Cell<Option<bool>>);

impl gpui::Global for StreamFade {}

pub fn install(cx: &mut App) {
    cx.set_global(StreamFade::default());
}

/// `getStreamFadePreference() === 'on'`; the fade is on by default.
pub fn enabled(cx: &mut App) -> bool {
    if let Some(known) = cx.global::<StreamFade>().0.get() {
        return known;
    }
    let prefs = cx.global_mut::<Preferences>();
    let stored = match prefs.text(KEY) {
        Ok(stored) => stored.map(str::to_owned),
        // As Cindy's `catch`: the default, and a later read asks again.
        Err(error) => {
            log::warn!(target: "stream-fade", "{error:#}");
            return true;
        }
    };
    let on = match stored.as_deref() {
        Some("on") => {
            // A stored default would hold back a later change of defaults.
            if let Err(error) = prefs.remove(KEY) {
                log::error!(target: "stream-fade", "{KEY} not removed: {error:#}");
            }
            true
        }
        Some("off") => false,
        _ => return true,
    };
    cx.global::<StreamFade>().0.set(Some(on));
    on
}

#[cfg(test)]
mod tests;
