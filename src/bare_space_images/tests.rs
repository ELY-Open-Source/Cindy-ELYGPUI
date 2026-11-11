use super::find;
use crate::{
    deep_link::tests::object,
    markdown_target::tests::golden,
    rich_answer::js::{Js, utf16_len},
};

#[test]
fn images_match_cindys() {
    golden("findBareSpaceLocalImages", |args| {
        let text = args[0].str().unwrap();
        let unit = |at: usize| Js::Num(utf16_len(&text[..at]) as f64);
        let image = |image: super::Image| {
            object(vec![
                ("start", unit(image.range.start)),
                ("end", unit(image.range.end)),
                ("alt", Js::Str(image.alt)),
                ("url", Js::Str(image.url)),
                ("title", image.title.map_or(Js::Null, Js::Str)),
            ])
        };
        Js::Arr(find(text).into_iter().map(image).collect())
    });
}
