//! Golden `streaming-throttle`: Cindy's hook mounted in jsdom on a set clock.

use serde_json::Value;

use super::Throttle;

#[test]
fn a_stream_is_shown_as_cindys_hook_shows_it() {
    let golden = include_str!("../../tests/fixtures/streaming-throttle/expected.json");
    let scenarios: Vec<(f64, Vec<Value>)> = serde_json::from_str(golden).unwrap();
    let text = |len: &Value| "x".repeat(len.as_u64().unwrap() as usize);
    let mut steps = 0;
    for (nth, (start, rows)) in scenarios.iter().enumerate() {
        let mut now = *start;
        let mut hook: Option<Throttle> = None;
        let (mut value, mut enabled) = (String::new(), false);
        for (at, row) in rows.iter().enumerate() {
            let [dt, len, on, fired, shown, due] = row.as_array().unwrap().as_slice() else {
                panic!("row shape");
            };
            let target = now + dt.as_f64().unwrap();
            if let Some(hook) = hook.as_mut() {
                while let Some(due) = hook.due().filter(|due| *due <= target) {
                    hook.fire(due);
                }
                let seen = hook.shown(&value, enabled).len();
                assert_eq!(Some(seen as u64), fired.as_u64(), "scenario {nth} row {at}");
            }
            now = target;
            (value, enabled) = (text(len), on.as_bool().unwrap());
            let hook = hook.get_or_insert_with(|| Throttle::new(&value));
            hook.render(&value, enabled, now);
            // The effect's state shows in the render it asks for.
            let seen = hook.shown(&value, enabled).len();
            assert_eq!(Some(seen as u64), shown.as_u64(), "scenario {nth} row {at}");
            assert_eq!(hook.due(), due.as_f64(), "scenario {nth} row {at}");
            steps += 1;
        }
    }
    assert!(steps > 2000);
}
