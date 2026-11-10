use super::{Atom, rows};

/// Every character 10 wide.
fn even(line: &str) -> Vec<Atom> {
    let atom = |(at, _)| Atom { at, advance: 10.0 };
    line.char_indices().map(atom).collect()
}

fn broken(line: &str, width: f32) -> Vec<&str> {
    let rows = rows(line, &even(line), width, &[]);
    rows.into_iter().map(|row| &line[row]).collect()
}

#[test]
fn a_line_breaks_between_words() {
    assert_eq!(broken("", 50.0), [""]);
    assert_eq!(broken("abc", 50.0), ["abc"]);
    assert_eq!(broken("aa bb cc", 50.0), ["aa bb", "cc"]);
    assert_eq!(broken("aa bb cc dd", 60.0), ["aa bb", "cc dd"]);
    assert_eq!(broken("aaaa bbbb", 40.0), ["aaaa", "bbbb"]);
}

#[test]
fn a_word_too_long_breaks_where_it_overflows() {
    assert_eq!(broken("abcdefgh", 30.0), ["abc", "def", "gh"]);
    assert_eq!(broken("ab cdefgh", 30.0), ["ab", "cde", "fgh"]);
}

#[test]
fn white_space_at_a_wrap_hangs() {
    assert_eq!(broken("aaa   bbb", 30.0), ["aaa", "bbb"]);
    assert_eq!(broken("aaa \t\u{3000} bbb", 30.0), ["aaa", "bbb"]);
    // White space that ends the line stays in its row and wraps nothing.
    assert_eq!(broken("aaa   ", 30.0), ["aaa   "]);
    assert_eq!(broken("   aaa", 30.0), ["", "aaa"]);
    // No-break space keeps its room.
    assert_eq!(broken("aaa\u{a0}\u{a0}b", 30.0), ["aaa", "\u{a0}\u{a0}b"]);
}

#[test]
fn a_chip_breaks_as_a_word() {
    let line = "aa \u{fffc} bb";
    let mut atoms = even(line);
    atoms[3].advance = 40.0;
    let rows = rows(line, &atoms, 60.0, &[]);
    let rows: Vec<&str> = rows.into_iter().map(|row| &line[row]).collect();
    assert_eq!(rows, ["aa", "\u{fffc}", "bb"]);
}

/// A shaped line of these glyphs, each a byte index and its x.
fn shaped(glyphs: &[(usize, f32)], width: f32) -> gpui::LineLayout {
    let glyph = |(index, x): &(usize, f32)| gpui::ShapedGlyph {
        id: gpui::GlyphId(0),
        position: gpui::point(gpui::px(*x), gpui::px(0.0)),
        index: *index,
        is_emoji: false,
    };
    let run = gpui::ShapedRun {
        font_id: gpui::FontId(0),
        glyphs: glyphs.iter().map(glyph).collect(),
    };
    gpui::LineLayout {
        width: gpui::px(width),
        runs: vec![run],
        len: glyphs.len(),
        ..Default::default()
    }
}

fn flat(layout: &gpui::LineLayout) -> Vec<(usize, f32)> {
    let atoms = super::atoms(layout);
    atoms.iter().map(|atom| (atom.at, atom.advance)).collect()
}

#[test]
fn atoms_read_in_byte_order_with_their_advances() {
    let plain = shaped(&[(0, 0.0), (1, 4.0), (2, 10.0)], 13.0);
    assert_eq!(flat(&plain), [(0, 4.0), (1, 6.0), (2, 3.0)]);
    // Right-to-left glyphs come from the right; each still ends where the next x starts.
    let hebrew = shaped(&[(0, 20.0), (2, 12.0), (4, 0.0)], 25.0);
    assert_eq!(flat(&hebrew), [(0, 5.0), (2, 8.0), (4, 12.0)]);
    // A mark on its letter's byte joins it.
    let marked = shaped(&[(0, 0.0), (0, 6.0), (3, 7.0)], 10.0);
    assert_eq!(flat(&marked), [(0, 7.0), (3, 3.0)]);
    assert_eq!(flat(&shaped(&[], 0.0)), []);
}

#[test]
fn a_clamped_row_ends_where_the_ellipsis_fits() {
    let line = "abcdef ghij";
    let atoms = even(line);
    let clamp = |row, ellipsis, width| super::clamp(&atoms, row, ellipsis, width);
    // Room to spare: the whole row, then the ellipsis.
    assert_eq!(clamp(0..6, 10.0, 70.0), 6);
    assert_eq!(clamp(0..6, 10.0, 69.0), 5);
    assert_eq!(clamp(0..6, 10.0, 45.0), 3);
    // Atoms of other rows take no room.
    assert_eq!(clamp(7..11, 10.0, 50.0), 11);
    assert_eq!(clamp(7..11, 10.0, 40.0), 10);
}

/// Blink's `word-break: break-all` over a path, probed in Cindy's Electron.
#[test]
fn an_image_path_breaks_where_blink_breaks_it() {
    let log = include_str!("../../../tests/fixtures/line-breaks/break-all.jsonl");
    for case in log.lines() {
        let case: serde_json::Value = serde_json::from_str(case).unwrap();
        let word = |key: &str| case[key].as_str().unwrap().to_owned();
        let (before, path, after) = (word("before"), word("path"), word("after"));
        let line = format!("{before}{path}{after}");
        let span = before.len()..before.len() + path.len();
        let units = |at: usize| line[..at].encode_utf16().count() as u64;
        let found: Vec<u64> = super::breaks(&line, &[span])
            .into_iter()
            .filter(|at| *at > 0 && *at < line.len())
            .map(units)
            .collect();
        let blink: Vec<u64> = case["breaks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|at| at.as_u64().unwrap())
            .collect();
        assert_eq!(found, blink, "{line}");
    }
}

#[test]
fn a_line_breaks_where_blink_breaks_it() {
    let breaks = |line: &str| super::breaks(line, &[]);
    // After a run of spaces whatever follows, which UAX 14 denies before `/` and `!`.
    assert_eq!(breaks("see  /home !"), [5, 11]);
    assert_eq!(breaks("a\tb"), [2]);
    assert_eq!(breaks("a\u{1}b"), [] as [usize; 0]);
    // The pair table: an address breaks after `?` and a hyphen, not after `/`, `.` or `:`.
    assert_eq!(breaks("a/b.c:d\\e"), [] as [usize; 0]);
    assert_eq!(breaks("a?b x-y"), [2, 4, 6]);
    // A hyphen before a digit is a minus unless a letter or digit stands before it.
    assert_eq!(breaks("a-1 -1 9-2"), [2, 4, 7, 9]);
    // A hyphen before a letter past ASCII is the Unicode rules' too: they read what stands before.
    let log = include_str!("../../../tests/fixtures/line-breaks/hyphen.jsonl");
    for case in log.lines() {
        let case: serde_json::Value = serde_json::from_str(case).unwrap();
        let line = case["text"].as_str().unwrap();
        let units = |at: usize| line[..at].encode_utf16().count() as u64;
        let found: Vec<u64> = breaks(line).into_iter().map(units).collect();
        let blink: Vec<u64> = case["breaks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|at| at.as_u64().unwrap())
            .collect();
        assert_eq!(found, blink, "{line}");
    }
    // A chip before the hyphen opens a word as a space does.
    assert_eq!(breaks("\u{fffc}-\u{e9}"), [3]);
    // Past U+00FF the Unicode rules decide.
    assert_eq!(breaks("\u{4f60}\u{597d}a"), [3, 6]);
    assert_eq!(breaks("a\u{2014}b"), [1, 4]);
}

#[test]
fn break_all_adds_the_table_s_breaks() {
    use super::break_all::breaks;
    // `+` is a letter here; a hyphen parts from the letter before it and the digit after.
    assert_eq!(breaks("a+b"), [1, 2]);
    assert_eq!(breaks("x-1"), [1, 2]);
    assert_eq!(breaks("$5%"), [] as [usize; 0]);
    assert_eq!(breaks("$%"), [1]);
    // A combining mark keeps the class before it.
    assert_eq!(breaks("a\u{301}b"), [3]);
    assert_eq!(breaks("a.b:c"), [2, 4]);
}
