//! Attention (emphasis, strong, optionally GFM strikethrough) occurs in the
//! [text][] content type.
//!
//! ## Grammar
//!
//! Attention sequences form with the following BNF
//! (<small>see [construct][crate::construct] for character groups</small>):
//!
//! ```bnf
//! attention_sequence ::= 1*'*' | 1*'_'
//! gfm_attention_sequence ::= 1*'~'
//! ```
//!
//! Sequences are matched together to form attention based on which character
//! they contain, how long they are, and what character occurs before and after
//! each sequence.
//! Otherwise they are turned into data.
//!
//! ## HTML
//!
//! When asterisk/underscore sequences match, and two markers can be “taken”
//! from them, they together relate to the `<strong>` element in HTML.
//! When one marker can be taken, they relate to the `<em>` element.
//! See [*§ 4.5.2 The `em` element*][html-em] and
//! [*§ 4.5.3 The `strong` element*][html-strong] in the HTML spec for more
//! info.
//!
//! When tilde sequences match, they together relate to the `<del>` element in
//! HTML.
//! See [*§ 4.7.2 The `del` element*][html-del] in the HTML spec for more info.
//!
//! ## Recommendation
//!
//! It is recommended to use asterisks for emphasis/strong attention when
//! writing markdown.
//!
//! There are some small differences in whether sequences can open and/or close
//! based on whether they are formed with asterisks or underscores.
//! Because underscores also frequently occur in natural language inside words,
//! while asterisks typically never do, `CommonMark` prohibits underscore
//! sequences from opening or closing when *inside* a word.
//!
//! Because asterisks can be used to form the most markdown constructs, using
//! them has the added benefit of making it easier to gloss over markdown: you
//! can look for asterisks to find syntax while not worrying about other
//! characters.
//!
//! For strikethrough attention, it is recommended to use two markers.
//! While `github.com` allows single tildes too, it technically prohibits it in
//! their spec.
//!
//! ## Tokens
//!
//! * [`Emphasis`][Name::Emphasis]
//! * [`EmphasisSequence`][Name::EmphasisSequence]
//! * [`EmphasisText`][Name::EmphasisText]
//! * [`GfmStrikethrough`][Name::GfmStrikethrough]
//! * [`GfmStrikethroughSequence`][Name::GfmStrikethroughSequence]
//! * [`GfmStrikethroughText`][Name::GfmStrikethroughText]
//! * [`Strong`][Name::Strong]
//! * [`StrongSequence`][Name::StrongSequence]
//! * [`StrongText`][Name::StrongText]
//!
//! > 👉 **Note**: while parsing, [`AttentionSequence`][Name::AttentionSequence]
//! > is used, which is later compiled away.
//!
//! ## References
//!
//! * [`attention.js` in `micromark`](https://github.com/micromark/micromark/blob/main/packages/micromark-core-commonmark/dev/lib/attention.js)
//! * [`micromark-extension-gfm-strikethrough`](https://github.com/micromark/micromark-extension-gfm-strikethrough)
//! * [*§ 6.2 Emphasis and strong emphasis* in `CommonMark`](https://spec.commonmark.org/0.31/#emphasis-and-strong-emphasis)
//! * [*§ 6.5 Strikethrough (extension)* in `GFM`](https://github.github.com/gfm/#strikethrough-extension-)
//!
//! [text]: crate::construct::text
//! [html-em]: https://html.spec.whatwg.org/multipage/text-level-semantics.html#the-em-element
//! [html-strong]: https://html.spec.whatwg.org/multipage/text-level-semantics.html#the-strong-element
//! [html-del]: https://html.spec.whatwg.org/multipage/edits.html#the-del-element

use crate::construct::gfm_autolink_literal::in_email;
use crate::event::{Event, Kind, Name, Point};
use crate::resolve::Name as ResolveName;
use crate::state::{Name as StateName, State};
use crate::subtokenize::Subresult;
use crate::tokenizer::Tokenizer;
use crate::util::char::{
    after_index as char_after_index, before_index as char_before_index, classify_opt,
    Kind as CharacterKind,
};
use alloc::{vec, vec::Vec};

/// Attentention sequence that we can take markers from.
#[derive(Debug)]
struct Sequence {
    /// Marker as a byte (`u8`) used in this sequence.
    marker: u8,
    /// We track whether sequences are in balanced events, and where those
    /// events start, so that one attention doesn’t start in say, one link, and
    /// end in another.
    stack: Vec<usize>,
    /// The index into events where this sequence’s `Enter` currently resides.
    index: usize,
    /// The (shifted) point where this sequence starts.
    start_point: Point,
    /// The (shifted) point where this sequence end.
    end_point: Point,
    /// The number of markers we can still use.
    size: usize,
    /// Whether this sequence can open attention.
    open: bool,
    /// Whether this sequence can close attention.
    close: bool,
    /// Cindy: whether a micromark chunk ends where this sequence ends.
    chunk_end: bool,
}

/// At start of attention.
///
/// ```markdown
/// > | **
///     ^
/// ```
pub fn start(tokenizer: &mut Tokenizer) -> State {
    // Cindy: micromark's email, begun before this underscore, has taken it.
    if tokenizer.current == Some(b'_')
        && tokenizer.parse_state.options.constructs.gfm_autolink_literal
        && (tokenizer.tokenize_state.label_spans.iter()).all(|span| span.1.is_some())
        && in_email(tokenizer)
    {
        return State::Nok;
    }

    // Emphasis/strong:
    if (tokenizer.parse_state.options.constructs.attention
        && matches!(tokenizer.current, Some(b'*' | b'_')))
        // GFM strikethrough:
        || (tokenizer.parse_state.options.constructs.gfm_strikethrough && tokenizer.current == Some(b'~'))
    {
        tokenizer.tokenize_state.marker = tokenizer.current.unwrap();
        tokenizer.enter(Name::AttentionSequence);
        State::Retry(StateName::AttentionInside)
    } else {
        State::Nok
    }
}

/// In sequence.
///
/// ```markdown
/// > | **
///     ^^
/// ```
pub fn inside(tokenizer: &mut Tokenizer) -> State {
    if tokenizer.current == Some(tokenizer.tokenize_state.marker) {
        tokenizer.consume();
        State::Next(StateName::AttentionInside)
    } else {
        tokenizer.exit(Name::AttentionSequence);
        tokenizer.register_resolver(ResolveName::Attention);
        tokenizer.tokenize_state.marker = 0;
        State::Ok
    }
}

/// Resolve sequences.
pub fn resolve(tokenizer: &mut Tokenizer) -> Option<Subresult> {
    // Find all sequences, gather info about them.
    let mut sequences = get_sequences(tokenizer);

    // Cindy: micromark resolves attention and strikethrough apart, the first it met first.
    let single = tokenizer.parse_state.options.gfm_strikethrough_single_tilde;
    let first = sequences
        .iter()
        .find(|sequence| {
            sequence.marker != b'~' || sequence.size == 2 || (sequence.size == 1 && single)
        })
        .is_some_and(|sequence| sequence.marker == b'~');

    for round in [false, true] {
        // Cindy: in a label attention comes first, as `insideSpan` lists it.
        let pick = |sequence: &Sequence| {
            (sequence.marker == b'~') == (round != (sequence.stack.is_empty() && first))
        };
        let end = sequences.len();
        pass(tokenizer, &mut sequences, 0, end, &pick);
    }

    // Mark remaining sequences as data.
    let mut index = 0;
    while index < sequences.len() {
        let sequence = &sequences[index];
        tokenizer.events[sequence.index].name = Name::Data;
        tokenizer.events[sequence.index + 1].name = Name::Data;
        index += 1;
    }

    tokenizer.map.consume(&mut tokenizer.events);
    None
}

/// Cindy: match the closing sequences `pick` takes in a range; gives the range's new end.
fn pass(
    tokenizer: &mut Tokenizer,
    sequences: &mut Vec<Sequence>,
    from: usize,
    mut to: usize,
    pick: &dyn Fn(&Sequence) -> bool,
) -> usize {
    let mut close = from;

    while close < to {
        let mut next_index = close + 1;

        // Find a sequence that can close.
        if sequences[close].close && pick(&sequences[close]) {
            let mut open = close;

            // Now walk back to find an opener.
            while open > from {
                open -= 1;

                let sequence_open = &sequences[open];
                let sequence_close = &sequences[close];

                // An opener matching our closer:
                if sequence_open.open
                    && sequence_close.marker == sequence_open.marker
                    && sequence_close.stack == sequence_open.stack
                {
                    // If the opening can close or the closing can open,
                    // and the close size *is not* a multiple of three,
                    // but the sum of the opening and closing size *is*
                    // multiple of three, then **don’t** match.
                    if (sequence_open.close || sequence_close.open)
                        && sequence_close.size % 3 != 0
                        && (sequence_open.size + sequence_close.size) % 3 == 0
                    {
                        continue;
                    }

                    // For GFM strikethrough:
                    // * both sequences must have the same size
                    // * more than 2 markers don’t work
                    // * one marker is prohibited by the spec, but supported by GH
                    if sequence_close.marker == b'~'
                        && (sequence_close.size != sequence_open.size
                            || sequence_close.size > 2
                            || sequence_close.size == 1
                                && !tokenizer.parse_state.options.gfm_strikethrough_single_tilde)
                    {
                        continue;
                    }

                    // Cindy: micromark resolves what the pair holds, then lets it go.
                    for strike in [false, true] {
                        let before = sequences.len();
                        pass(tokenizer, sequences, open + 1, close, &|sequence| {
                            (sequence.marker == b'~') == strike
                        });
                        close -= before - sequences.len();
                        to -= before - sequences.len();
                    }

                    // We found a match!
                    let before = sequences.len();
                    next_index = match_sequences(tokenizer, sequences, open, close);
                    to -= before - sequences.len();

                    break;
                }
            }
        }

        close = next_index;
    }

    to
}

/// Get sequences.
fn get_sequences(tokenizer: &mut Tokenizer) -> Vec<Sequence> {
    let mut index = 0;
    let mut stack = vec![];
    let mut sequences = vec![];

    while index < tokenizer.events.len() {
        let enter = &tokenizer.events[index];

        if enter.name == Name::AttentionSequence {
            if enter.kind == Kind::Enter {
                let end = index + 1;
                let exit = &tokenizer.events[end];

                let marker = tokenizer.parse_state.bytes[enter.point.index];
                let before_char = char_before_index(tokenizer.parse_state.bytes, enter.point.index);
                let before = classify_opt(before_char);
                let after_char = char_after_index(tokenizer.parse_state.bytes, exit.point.index);
                let after = classify_opt(after_char);
                let open = after == CharacterKind::Other
                    || (after == CharacterKind::Punctuation && before != CharacterKind::Other)
                    // For regular attention markers (not strikethrough), the
                    // other attention markers can be used around them
                    || (marker != b'~' && matches!(after_char, Some('*' | '_')))
                    || (marker != b'~' && tokenizer.parse_state.options.constructs.gfm_strikethrough && matches!(after_char, Some('~')));
                let close = before == CharacterKind::Other
                    || (before == CharacterKind::Punctuation && after != CharacterKind::Other)
                    || (marker != b'~' && matches!(before_char, Some('*' | '_')))
                    || (marker != b'~'
                        && tokenizer.parse_state.options.constructs.gfm_strikethrough
                        && matches!(before_char, Some('~')));

                // Cindy: micromark's attention extensions judge `*` and `_` themselves.
                let flanks = tokenizer
                    .parse_state
                    .options
                    .attention_flanks
                    .as_ref()
                    .filter(|_| marker != b'~')
                    .map(|flanks| {
                        let at = (enter.point.index, exit.point.index);
                        let slice = tokenizer
                            .slices
                            .iter()
                            .find(|(start, end)| *start <= at.0 && at.1 <= *end)
                            .expect("a sequence lies in a pushed slice");
                        flanks(tokenizer.parse_state.bytes, *slice, at)
                    });
                let (open, close) = flanks.unwrap_or(if marker == b'_' {
                    (
                        open && (before != CharacterKind::Other || !close),
                        close && (after != CharacterKind::Other || !open),
                    )
                } else {
                    (open, close)
                });

                sequences.push(Sequence {
                    index,
                    stack: stack.clone(),
                    start_point: enter.point.clone(),
                    end_point: exit.point.clone(),
                    size: exit.point.index - enter.point.index,
                    open,
                    close,
                    marker,
                    chunk_end: tokenizer
                        .parse_state
                        .options
                        .chunk_ends
                        .contains(&exit.point.index),
                });
            }
        } else if enter.kind == Kind::Enter {
            stack.push(index);
        } else {
            stack.pop();
        }

        index += 1;
    }

    sequences
}

/// Match two sequences.
#[allow(clippy::too_many_lines)]
fn match_sequences(
    tokenizer: &mut Tokenizer,
    sequences: &mut Vec<Sequence>,
    open: usize,
    close: usize,
) -> usize {
    // Where to move to next.
    // Stay on this closing sequence for the next iteration: it
    // might close more things.
    // It’s changed if sequences are removed.
    let mut next = close;

    // Number of markers to use from the sequence.
    let take = if sequences[open].size > 1 && sequences[close].size > 1 {
        2
    } else {
        1
    };

    // We’re *on* a closing sequence, with a matching opening
    // sequence.
    // Now we make sure that we can’t have misnested attention:
    //
    // ```html
    // <em>a <strong>b</em> c</strong>
    // ```
    //
    // Do that by marking everything between it as no longer
    // possible to open anything.
    // Theoretically we should mark as `close: false` too, but
    // we don’t look for closers backwards, so it’s not needed.
    let mut between = open + 1;

    while between < close {
        sequences[between].open = false;
        // Cindy: a later pass finds no closer in a matched pair.
        sequences[between].close = false;
        between += 1;
    }

    let (group_name, seq_name, text_name) = if sequences[open].marker == b'~' {
        (
            Name::GfmStrikethrough,
            Name::GfmStrikethroughSequence,
            Name::GfmStrikethroughText,
        )
    } else if take == 1 {
        (Name::Emphasis, Name::EmphasisSequence, Name::EmphasisText)
    } else {
        (Name::Strong, Name::StrongSequence, Name::StrongText)
    };
    let open_index = sequences[open].index;
    let close_index = sequences[close].index;
    let open_exit = sequences[open].end_point.clone();
    let close_enter = sequences[close].start_point.clone();

    // No need to worry about `VS`, because sequences are only actual characters.
    sequences[open].size -= take;
    sequences[close].size -= take;
    sequences[open].end_point.column -= take;
    sequences[open].end_point.index -= take;
    sequences[close].start_point.column += take;
    sequences[close].start_point.index += take;

    // Opening.
    tokenizer.map.add_before(
        // Add after the current sequence (it might remain).
        open_index + 2,
        0,
        vec![
            Event {
                kind: Kind::Enter,
                name: group_name.clone(),
                point: sequences[open].end_point.clone(),
                link: None,
            },
            Event {
                kind: Kind::Enter,
                name: seq_name.clone(),
                point: sequences[open].end_point.clone(),
                link: None,
            },
            Event {
                kind: Kind::Exit,
                name: seq_name.clone(),
                point: open_exit.clone(),
                link: None,
            },
            Event {
                kind: Kind::Enter,
                name: text_name.clone(),
                point: open_exit,
                link: None,
            },
        ],
    );
    // Closing.
    tokenizer.map.add(
        close_index,
        0,
        vec![
            Event {
                kind: Kind::Exit,
                name: text_name,
                point: close_enter.clone(),
                link: None,
            },
            Event {
                kind: Kind::Enter,
                name: seq_name.clone(),
                point: close_enter,
                link: None,
            },
            Event {
                kind: Kind::Exit,
                name: seq_name,
                point: sequences[close].start_point.clone(),
                link: None,
            },
            Event {
                kind: Kind::Exit,
                name: group_name,
                point: sequences[close].start_point.clone(),
                link: None,
            },
        ],
    );

    // Remove closing sequence if fully used.
    if sequences[close].size == 0 {
        sequences.remove(close);
        tokenizer.map.add(close_index, 2, vec![]);
    } else {
        // Shift remaining closing sequence forward.
        // Do it here because a sequence can open and close different
        // other sequences, and the remainder can be on any side or
        // somewhere in the middle.
        tokenizer.events[close_index].point = sequences[close].start_point.clone();
    }

    if sequences[open].size == 0 {
        sequences.remove(open);
        tokenizer.map.add(open_index, 2, vec![]);
        // Everything shifts one to the left, account for it in next iteration.
        next -= 1;
    } else if !sequences[open].chunk_end {
        // Cindy: micromark's `movePoint` leaves the rest whole at a chunk's end.
        tokenizer.events[open_index + 1].point = sequences[open].end_point.clone();
    }

    next
}
