use super::{Face, quote};
use crate::chat_quotes::Quote;

#[test]
fn a_quote_chip_reads_on_one_line_and_tells_the_whole() {
    let said = Quote {
        text: " a\u{a0}b\u{3000}\nc\u{feff}".to_owned(),
        source_path: Some("src/a.rs".to_owned()),
        start_line: Some(3.0),
        ..Default::default()
    };
    let chip = quote(&said, true);
    // JavaScript's white space, the no-break and wide spaces too, folds to one space.
    assert_eq!(chip.label.as_ref(), "a b c");
    assert_eq!(chip.name.as_ref(), said.text);
    assert_eq!(chip.tip.as_ref(), said.text);
    assert!(chip.named && chip.island);
    let Face::Quote(Some((path, label))) = chip.face else {
        panic!("the source is told");
    };
    assert_eq!((path.as_ref(), label.as_ref()), ("src/a.rs", "a.rs:L3"));
}
