use super::find;
use crate::{
    markdown_target::tests::golden,
    rich_answer::js::{Js, utf16_len},
};

#[test]
fn paths_match_cindys() {
    golden("findPathMatches", |args| {
        let text = args[0].str().unwrap();
        let unit = |at: usize| Js::Num(utf16_len(&text[..at]) as f64);
        let pair =
            |range: std::ops::Range<usize>| Js::Arr(vec![unit(range.start), unit(range.end)]);
        Js::Arr(find(text).into_iter().map(pair).collect())
    });
}
