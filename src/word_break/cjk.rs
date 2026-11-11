//! ICU's `CjkBreakEngine::divideUpDictionaryRange` over ICU4X's `cjdict`, no phrase breaking.

use icu_collections::char16trie::Char16Trie;
use icu_normalizer::{ComposingNormalizerBorrowed, DecomposingNormalizerBorrowed};
use icu_properties::{CodePointMapData, props::CanonicalCombiningClass};
use icu_provider::prelude::*;
use icu_segmenter::provider::{Baked, DictionaryBreakData, SegmenterDictionaryAutoV2};

thread_local! {
    /// `cjdict`, each word with its cost.
    static WORDS: DataPayload<SegmenterDictionaryAutoV2> = {
        let name = DataMarkerAttributes::from_str_or_panic("cjdict");
        let request = DataRequest {
            id: DataIdentifierBorrowed::for_marker_attributes(name),
            ..Default::default()
        };
        let loaded = DataProvider::<SegmenterDictionaryAutoV2>::load(&Baked, request);
        loaded.expect("ICU4X's data holds cjdict").payload
    };
}

/// `isKatakana`.
fn katakana(c: char) -> bool {
    matches!(c, '\u{30a1}'..='\u{30fe}' | '\u{ff66}'..='\u{ff9f}') && c != '\u{30fb}'
}

/// `getKatakanaCost`.
fn katakana_cost(length: usize) -> u32 {
    const COSTS: [u32; 9] = [8192, 984, 408, 240, 204, 252, 300, 372, 480];
    COSTS.get(length).copied().unwrap_or(8192)
}

/// `UCharsDictionaryMatcher::matches`: each word that starts `run`, its length and cost.
fn matches(run: &[char], trie: &Char16Trie) -> Vec<(usize, u32)> {
    let (mut found, mut units) = (Vec::new(), 0);
    let mut cursor = trie.cursor();
    for (count, c) in run.iter().enumerate() {
        cursor.step(*c);
        if let Some(cost) = cursor.value() {
            found.push((count + 1, cost as u32));
        }
        units += c.len_utf16();
        if cursor.is_empty() || units >= 20 {
            break;
        }
    }
    found
}

/// The cheapest way to cut `run` into words: where each word ends, the run's end last.
fn divide(run: &[char], trie: &Char16Trie) -> Vec<usize> {
    let n = run.len();
    let (mut best, mut prev) = (vec![u32::MAX; n + 1], vec![0; n + 1]);
    best[0] = 0;
    let mut cheaper = |from: usize, length: usize, cost: u32, best: &mut Vec<u32>| {
        let cost = best[from].wrapping_add(cost);
        if cost < best[from + length] {
            (best[from + length], prev[from + length]) = (cost, from);
        }
    };
    for i in 0..n {
        if best[i] == u32::MAX {
            continue;
        }
        let mut found = matches(&run[i..], trie);
        let hangul = ('\u{ac00}'..='\u{d7a3}').contains(&run[i]);
        if found.first().is_none_or(|(length, _)| *length != 1) && !hangul {
            found.push((1, 255));
        }
        for (length, cost) in found {
            cheaper(i, length, cost, &mut best);
        }
        let starts = katakana(run[i]) && !(i > 0 && katakana(run[i - 1]));
        let length = run[i..]
            .iter()
            .take(20)
            .take_while(|c| katakana(**c))
            .count();
        if starts && length < 20 {
            cheaper(i, length, katakana_cost(length), &mut best);
        }
    }
    let mut cuts = vec![n];
    let mut at = if best[n] == u32::MAX { 0 } else { prev[n] };
    while at > 0 {
        cuts.push(at);
        at = prev[at];
    }
    cuts.reverse();
    cuts
}

/// `run` as NFKC, and the character of `run` each of its characters came from.
fn normalized(run: &[char]) -> (Vec<char>, Vec<usize>) {
    let nfkc = ComposingNormalizerBorrowed::new_nfkc();
    let text: String = run.iter().collect();
    if nfkc.is_normalized(&text) {
        return (run.to_vec(), (0..=run.len()).collect());
    }
    let nfkd = DecomposingNormalizerBorrowed::new_nfkd();
    let class = CodePointMapData::<CanonicalCombiningClass>::new();
    // `hasBoundaryBefore`, for the characters a CJK run holds.
    let starts = |c: char| {
        let first = nfkd.normalize(c.encode_utf8(&mut [0; 4])).chars().next();
        first.is_some_and(|first| class.get(first) == CanonicalCombiningClass::NotReordered)
    };
    let (mut chars, mut map, mut from) = (Vec::new(), Vec::new(), 0);
    while from < run.len() {
        let more = run[from + 1..].iter().take_while(|c| !starts(**c)).count();
        let fragment: String = run[from..=from + more].iter().collect();
        for c in nfkc.normalize(&fragment).chars() {
            chars.push(c);
            map.push(from);
        }
        from += more + 1;
    }
    map.push(run.len());
    (chars, map)
}

/// The characters of `run` a word starts at, its first apart.
pub fn inner(run: &[char]) -> Vec<usize> {
    let (chars, map) = normalized(run);
    let ends = WORDS.with(|words| match words.get() {
        DictionaryBreakData::Char16Trie(trie) => divide(&chars, trie),
        _ => panic!("cjdict is no UCharsTrie"),
    });
    let mut out: Vec<usize> = Vec::new();
    for cut in ends {
        let at = map[cut];
        if at > 0 && at < run.len() && out.last().is_none_or(|last| at > *last) {
            out.push(at);
        }
    }
    out
}
