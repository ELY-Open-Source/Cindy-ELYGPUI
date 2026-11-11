use crate::{
    markdown::image_targets,
    markdown_media,
    rich_answer::js::{self, Js},
};

fn golden() -> Js {
    js::parse(include_str!(
        "../../tests/fixtures/image-targets/expected.json"
    ))
    .unwrap()
}

#[test]
fn a_remote_media_url_unwraps_and_a_managed_video_is_told_as_cindys() {
    let golden = golden();
    let cases = golden.obj().unwrap()["urls"].arr().unwrap();
    assert!(cases.len() > 40);
    for case in cases {
        let [url, orig, video] = case.arr().unwrap() else {
            panic!("a case of three");
        };
        let url = url.str().unwrap();
        assert_eq!(super::orig(url).as_deref(), orig.str(), "{url}");
        let told = markdown_media::is_managed_video(url);
        assert_eq!(Js::Bool(told), *video, "{url}");
    }
}

#[test]
fn a_text_s_image_targets_are_cindys() {
    let golden = golden();
    let cases = golden.obj().unwrap()["texts"].arr().unwrap();
    assert!(cases.len() > 25);
    for case in cases {
        let [text, targets] = case.arr().unwrap() else {
            panic!("a case of two");
        };
        let text = text.str().unwrap();
        let want: Vec<&str> = targets
            .arr()
            .unwrap()
            .iter()
            .map(|t| t.str().unwrap())
            .collect();
        assert_eq!(image_targets::extract(text), want, "{text}");
    }
}

#[test]
fn the_cache_reads_a_changed_content_again() {
    let cache = image_targets::Cache::default();
    let one = cache.targets("assistant-1", "![one](cindy-media://one.png)");
    assert_eq!(one, ["cindy-media://one.png"]);
    assert_eq!(
        cache.targets("assistant-1", "![one](cindy-media://one.png)"),
        one
    );
    let two = cache.targets("assistant-1", "![two](cindy-media://two.png)");
    assert_eq!(two, ["cindy-media://two.png"]);
    assert_eq!(
        cache.targets("assistant-2", "plain text"),
        Vec::<String>::new()
    );
}
