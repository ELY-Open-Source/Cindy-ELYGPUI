use std::sync::Arc;

use super::{Event, Fit, Frame, Source, Video, receive, wanted};
use crate::wallpaper::Motion;

#[test]
fn decodes_at_the_cover_size_and_coarser_under_blur() {
    let sharp = Fit {
        target: (1000, 500),
        sigma: 0.0,
    };
    assert_eq!(sharp.scaled((2304, 1440)), (1000, 625), "covers the box");
    assert_eq!(
        sharp.scaled((640, 360)),
        (640, 360),
        "never past the source"
    );
    let blurred = Fit {
        target: (1000, 500),
        sigma: 8.0,
    };
    assert_eq!(blurred.scaled((2304, 1440)), (250, 157));
    assert_eq!(blurred.frame_sigma(250, (2304, 1440)), 2.0);
}

#[test]
fn plays_as_wallpaper_video_decides() {
    let custom = Source::Custom(Arc::new(vec![1]));
    let official = Source::Official(Arc::new(vec![2]));
    let cases = [
        (&custom, Motion::Dynamic, false, true, Some(true)),
        (&custom, Motion::Static, false, true, Some(false)),
        (&custom, Motion::Dynamic, true, true, Some(false)),
        (&custom, Motion::Dynamic, false, false, Some(false)),
        (&official, Motion::Dynamic, false, true, Some(true)),
        (&official, Motion::Dynamic, false, false, Some(false)),
        (&official, Motion::Static, false, true, None),
        (&official, Motion::Dynamic, true, true, None),
    ];
    for (source, motion, reduced, visible, want) in cases {
        let got = wanted(source, motion, reduced, visible);
        assert_eq!(
            got, want,
            "{source:?} {motion:?} reduced {reduced} visible {visible}"
        );
    }
}

#[gpui::test]
fn a_dropped_players_frames_are_ignored(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        cx.set_global(crate::motion::ReducedMotion(false));
        let mut video = Video::new();
        video.generation = 2;
        cx.set_global(video);
        let frame = || Event::Frame(Frame(image::RgbaImage::new(2, 2)));
        receive(frame(), 1, cx);
        assert!(cx.global::<Video>().frame.is_none());
        receive(frame(), 2, cx);
        assert!(cx.global::<Video>().frame.is_some());
    });
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn first_event(bytes: &[u8]) -> Event {
    let (send, events) = std::sync::mpsc::channel();
    let fit = Fit {
        target: (32, 20),
        sigma: 0.0,
    };
    let player = super::Player::open(bytes, fit, move |event| {
        let _ = send.send(event);
    })
    .unwrap();
    player.play(false).unwrap();
    let event = events
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    drop(player);
    event
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn the_decoder_plays_the_bytes_that_were_checked() {
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/video/tiny.mp4");
    let dir = tempfile::tempdir().unwrap();
    let hash = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let bucket = dir.path().join("cindy-media/client-wallpaper/01");
    std::fs::create_dir_all(&bucket).unwrap();
    let file = bucket.join(format!("{hash}.mp4"));
    std::fs::copy(fixture, &file).unwrap();
    let url = format!("cindy-media://client-wallpaper/{hash}.mp4");
    let bytes = crate::wallpaper::media::custom_media(dir.path(), &url).unwrap();
    std::fs::write(&file, b"replaced after the read").unwrap();
    let image = match first_event(&bytes) {
        Event::Frame(Frame(image)) => image,
        Event::Failed(reason) => panic!("the checked bytes play: {reason}"),
    };
    assert_eq!(
        image.dimensions(),
        (64, 40),
        "the first frame comes at source size"
    );
    let Event::Failed(reason) = first_event(b"not a video") else {
        panic!("a broken video fails");
    };
    assert!(!reason.contains('/') && !reason.contains(hash), "{reason}");
}
