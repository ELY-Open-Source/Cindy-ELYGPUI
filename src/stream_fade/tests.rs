use gpui::TestAppContext;

use super::{KEY, enabled, install};
use crate::preferences::Preferences;

fn start(cx: &mut TestAppContext, stored: Option<&str>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    cx.update(|cx| {
        let mut prefs = Preferences::load(dir.path()).unwrap();
        if let Some(value) = stored {
            prefs.set(KEY, value).unwrap();
        }
        cx.set_global(prefs);
        install(cx);
    });
    dir
}

fn stored(cx: &mut TestAppContext) -> Option<String> {
    cx.update(|cx| {
        let prefs = cx.global::<Preferences>();
        prefs.text(KEY).unwrap().map(str::to_owned)
    })
}

#[gpui::test]
fn the_fade_is_on_until_turned_off(cx: &mut TestAppContext) {
    let _dir = start(cx, None);
    assert!(cx.update(enabled));
    cx.update(|cx| cx.global_mut::<Preferences>().set(KEY, "off").unwrap());
    // Nothing settled the first read, so the next one reads the store again.
    assert!(!cx.update(enabled));
}

#[gpui::test]
fn off_is_kept_in_memory_and_in_the_store(cx: &mut TestAppContext) {
    let _dir = start(cx, Some("off"));
    assert!(!cx.update(enabled));
    assert_eq!(stored(cx).as_deref(), Some("off"));
    cx.update(|cx| cx.global_mut::<Preferences>().remove(KEY).unwrap());
    assert!(!cx.update(enabled));
}

#[gpui::test]
fn a_stored_default_is_cleared(cx: &mut TestAppContext) {
    let _dir = start(cx, Some("on"));
    assert!(cx.update(enabled));
    assert_eq!(stored(cx), None);
    cx.update(|cx| cx.global_mut::<Preferences>().set(KEY, "off").unwrap());
    assert!(cx.update(enabled));
}

#[gpui::test]
fn an_unknown_value_reads_as_the_default(cx: &mut TestAppContext) {
    let _dir = start(cx, Some("sometimes"));
    assert!(cx.update(enabled));
    assert_eq!(stored(cx).as_deref(), Some("sometimes"));
}
