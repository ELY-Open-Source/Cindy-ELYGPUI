//! maker-shared `goalVerdict.ts`: the `/goal` block a reply ends with, left out of the shown text.

use std::sync::LazyLock;

use regex::Regex;

use crate::{js_re, theme_import::color::js_space};

static FENCED: LazyLock<Regex> = LazyLock::new(|| {
    let source = r#"\n?\s*```(?:json|jsonc)?\s*\{[^{}]*"goal_(?:status|setup)"[^{}]*\}\s*```\s*$"#;
    js_re::regex(source, true)
});
static BARE: LazyLock<Regex> =
    LazyLock::new(|| js_re::regex(r#"\n?\s*\{[^{}]*"goal_(?:status|setup)"[^{}]*\}\s*$"#, true));

/// `stripGoalVerdictBlock`: the text without its trailing fenced or bare block.
pub fn strip(content: &str) -> &str {
    if !content.contains("goal_status") && !content.contains("goal_setup") {
        return content;
    }
    let found = FENCED.find(content).or_else(|| BARE.find(content));
    match found {
        Some(found) => content[..found.start()].trim_end_matches(js_space),
        None => content,
    }
}

#[cfg(test)]
mod tests {
    use crate::rich_answer::js;

    #[test]
    fn the_trailing_block_is_stripped_as_cindy_strips_it() {
        let all = include_str!("../tests/fixtures/goal-verdict/expected.json");
        let all = js::parse(all).unwrap();
        let cases = all.obj().unwrap()["stripGoalVerdictBlock"].arr().unwrap();
        for case in cases {
            let [args, expected] = case.arr().unwrap() else {
                panic!("case shape")
            };
            let content = args.arr().unwrap()[0].str().unwrap();
            assert_eq!(
                super::strip(content),
                expected.str().unwrap(),
                "{content:?}"
            );
        }
        assert!(cases.len() > 20);
    }
}
