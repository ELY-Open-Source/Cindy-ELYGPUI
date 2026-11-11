//! Infer things from events.
//!
//! Used to share between `to_html` and `to_mdast`.

use crate::event::{Event, Kind, Name};
use crate::mdast::AlignKind;
use alloc::{vec, vec::Vec};

/// Figure out if a list is spread or not.
///
/// When `include_items: true` is passed, infers whether the list as a whole
/// is “loose”.
pub fn list_loose(events: &[Event], mut index: usize, include_items: bool) -> bool {
    let mut balance = 0;
    let name = &events[index].name;
    debug_assert!(
        matches!(name, Name::ListOrdered | Name::ListUnordered),
        "expected list"
    );

    while index < events.len() {
        let event = &events[index];

        if event.kind == Kind::Enter {
            balance += 1;

            if include_items
                && balance == 2
                && event.name == Name::ListItem
                && list_item_loose(events, index)
            {
                return true;
            }
        } else {
            balance -= 1;

            if balance == 1 && event.name == Name::BlankLineEnding {
                // Blank line directly after item, which is just a prefix.
                //
                // ```markdown
                // > | -␊
                //      ^
                //   | - a
                // ```
                let mut at_empty_list_item = false;
                // Blank line at block quote prefix:
                //
                // ```markdown
                // > | * >␊
                //        ^
                //   | * a
                // ```
                let mut at_empty_block_quote = false;

                // List.
                let mut before = index - 2;

                if events[before].name == Name::ListItem {
                    before -= 1;

                    if events[before].name == Name::SpaceOrTab {
                        before -= 2;
                    }

                    if events[before].name == Name::BlockQuote
                        && events[before - 1].name == Name::BlockQuotePrefix
                    {
                        at_empty_block_quote = true;
                    } else if events[before].name == Name::ListItemPrefix {
                        at_empty_list_item = true;
                    }
                }

                if !at_empty_list_item && !at_empty_block_quote {
                    return true;
                }
            }

            // Done.
            if balance == 0 && event.name == *name {
                break;
            }
        }

        index += 1;
    }

    false
}

/// Figure out if an item is spread or not.
pub fn list_item_loose(events: &[Event], mut index: usize) -> bool {
    debug_assert!(
        matches!(events[index].name, Name::ListItem),
        "expected list item"
    );
    let mut balance = 0;

    while index < events.len() {
        let event = &events[index];

        if event.kind == Kind::Enter {
            balance += 1;
        } else {
            balance -= 1;

            if balance == 1 && event.name == Name::BlankLineEnding {
                // Blank line directly after a prefix:
                //
                // ```markdown
                // > | -␊
                //      ^
                //   |   a
                // ```
                let mut at_prefix = false;

                // List item.
                let mut before = index - 2;

                if events[before].name == Name::SpaceOrTab {
                    before -= 2;
                }

                if events[before].name == Name::ListItemPrefix {
                    at_prefix = true;
                }

                if !at_prefix {
                    return true;
                }
            }

            // Done.
            if balance == 0 && event.name == Name::ListItem {
                break;
            }
        }

        index += 1;
    }

    false
}

/// Cindy: micromark's tokens, as its `subtokenize` and `prepareList` read them.
#[derive(Clone, Copy)]
enum Token {
    /// A list's enter, with its event.
    List(usize),
    ListEnd,
    Quote,
    QuoteEnd,
    /// `listItemPrefix`, with its item's event.
    Item(usize),
    /// A line ending, and whether it is blank.
    Ending(bool),
    /// `linePrefix` or `listItemIndent`.
    Space,
    QuotePrefix,
    Other,
}

/// Cindy: micromark's `subtokenize`: a container's exit moves before the line endings it follows.
fn close(tokens: &mut Vec<Token>, exit: Token) {
    let mut at = tokens.len();
    let mut line = None;
    while at > 0 {
        at -= 1;
        match tokens[at] {
            Token::Ending(_) => {
                if let Some(line) = line {
                    tokens[line] = Token::Ending(true);
                }
                tokens[at] = Token::Ending(false);
                line = Some(at);
            }
            Token::Space => {}
            _ => break,
        }
    }
    tokens.insert(line.unwrap_or(tokens.len()), exit);
}

/// Cindy: the containers closing before `after`, as the port left them.
struct Closing<'a> {
    /// Whether the last to close is an item, which a sibling may follow.
    item: bool,
    /// Whether a quote closes, which a blank line ends.
    quote: bool,
    /// The containers that stay open.
    open: &'a [Name],
    /// Where the document's last enter is.
    last: usize,
}

/// Cindy: where micromark exits them, and whether the exits found there close with them.
fn exits_at(events: &[Event], after: usize, closing: &Closing) -> (usize, bool) {
    let mut start = after;
    if start < events.len()
        && matches!(
            events[start].name,
            Name::LineEnding | Name::BlankLineEnding
        )
    {
        start += 2;
    }

    loop {
        let mut index = start;
        let mut cursor = 0;
        // Whether spaces no item took are left to the flow.
        let mut flow = false;
        while index < events.len() {
            if events[index].name == Name::BlockQuotePrefix {
                let quote = closing.open[cursor..]
                    .iter()
                    .position(|name| *name == Name::BlockQuote);
                cursor += quote.map_or(0, |quote| quote + 1);
                // The spaces before a quote's marker are its own.
                flow = false;
                while events[index].kind == Kind::Enter
                    || events[index].name != Name::BlockQuotePrefix
                {
                    index += 1;
                }
            } else if events[index].name != Name::SpaceOrTab {
                break;
            } else if events[index].kind == Kind::Enter {
                if closing.open.get(cursor) == Some(&Name::ListItem) {
                    cursor += 1;
                } else {
                    flow = true;
                }
            }
            index += 1;
        }

        let end = index == events.len() || index > closing.last;
        if end {
            return if closing.quote && flow {
                (start, false)
            } else {
                (index, false)
            };
        }
        let event = &events[index];
        if !closing.quote && event.name == Name::BlankLineEnding {
            start = index + 2;
            continue;
        }
        // More close on this line, a blank one: they close with the items it kept.
        if event.kind == Kind::Exit {
            return if closing.quote {
                (start, false)
            } else {
                (index, true)
            };
        }
        let container = match event.name {
            Name::BlockQuote
            | Name::ListOrdered
            | Name::ListUnordered
            | Name::GfmFootnoteDefinition => true,
            Name::ListItem => !closing.item,
            _ => false,
        };
        return (if container { index } else { start }, false);
    }
}

/// Cindy: whether only spaces lie between `index` and the end of its line.
fn blank(events: &[Event], mut index: usize) -> bool {
    while index < events.len() && events[index].name == Name::SpaceOrTab {
        index += 1;
    }
    // At the end of the document the containers exit there.
    index == events.len()
        || events[index].name == Name::BlankLineEnding
        || events[index].kind == Kind::Exit
}

/// Cindy: the events as micromark orders them, the exits of containers where it leaves them.
fn tokens(events: &[Event]) -> Vec<Token> {
    let mut tokens = Vec::new();
    // The containers a line continues, and the one its next prefix belongs to.
    let mut open = Vec::new();
    let mut cursor = 0;
    let mut held = Vec::new();
    let mut at = 0;
    let mut join = false;
    let last = events
        .iter()
        .rposition(|event| event.kind == Kind::Enter)
        .unwrap_or(0);
    let mut item = 0;
    let mut prefix = false;
    let mut index = 0;

    while index < events.len() {
        if index == at && !join {
            held.drain(..).for_each(|exit| close(&mut tokens, exit));
        }
        let event = &events[index];
        let enter = event.kind == Kind::Enter;
        index += 1;
        match event.name {
            Name::BlockQuotePrefix | Name::ListItemPrefix => {
                prefix = enter;
                if !enter {
                    continue;
                }
                if event.name == Name::ListItemPrefix {
                    tokens.push(Token::Item(item));
                } else {
                    let quote = open[cursor..]
                        .iter()
                        .position(|name| *name == Name::BlockQuote);
                    cursor += quote.map_or(0, |quote| quote + 1);
                    tokens.push(Token::QuotePrefix);
                }
            }
            _ if prefix => {}
            Name::ListItem | Name::BlockQuote | Name::GfmFootnoteDefinition if enter => {
                open.push(event.name.clone());
                match event.name {
                    Name::ListItem => item = index - 1,
                    Name::BlockQuote => tokens.push(Token::Quote),
                    _ => tokens.push(Token::Other),
                }
            }
            Name::ListOrdered | Name::ListUnordered if enter => tokens.push(Token::List(index - 1)),
            Name::ListItem
            | Name::BlockQuote
            | Name::GfmFootnoteDefinition
            | Name::ListOrdered
            | Name::ListUnordered => {
                match event.name {
                    Name::ListItem => {}
                    Name::BlockQuote => held.push(Token::QuoteEnd),
                    Name::GfmFootnoteDefinition => held.push(Token::Other),
                    _ => held.push(Token::ListEnd),
                }
                if !matches!(event.name, Name::ListOrdered | Name::ListUnordered) {
                    open.pop();
                }
                let ended = index == events.len()
                    || events[index].kind == Kind::Enter
                    || !matches!(
                        events[index].name,
                        Name::ListItem
                            | Name::BlockQuote
                            | Name::GfmFootnoteDefinition
                            | Name::ListOrdered
                            | Name::ListUnordered
                    );
                if ended {
                    let closing = Closing {
                        item: event.name == Name::ListItem,
                        quote: held.iter().any(|exit| matches!(exit, Token::QuoteEnd)),
                        open: &open,
                        last,
                    };
                    (at, join) = exits_at(events, index, &closing);
                }
            }
            Name::LineEnding | Name::BlankLineEnding => {
                cursor = 0;
                if enter {
                    tokens.push(Token::Ending(event.name == Name::BlankLineEnding));
                }
            }
            Name::SpaceOrTab => {
                if !enter {
                    continue;
                }
                // A definition's indent stops micromark's walks; a blank line has none.
                let indent = open.get(cursor) == Some(&Name::GfmFootnoteDefinition)
                    && !blank(events, index - 1);
                if indent || open.get(cursor) == Some(&Name::ListItem) {
                    cursor += 1;
                }
                tokens.push(if indent { Token::Other } else { Token::Space });
            }
            _ if matches!(tokens.last(), Some(Token::Other)) => {}
            _ => tokens.push(Token::Other),
        }
    }

    held.drain(..).for_each(|exit| close(&mut tokens, exit));
    tokens
}

/// Cindy: mdast-util-from-markdown's `prepareList` over the list from `start` to `end`.
fn prepare(tokens: &[Token], start: usize, end: usize, spread: &mut [bool]) {
    let Token::List(list) = tokens[start] else {
        unreachable!("expected list")
    };
    let mut balance = -1;
    let mut item = None;
    let mut marker = false;
    let mut blank = None;

    for at in start..=end {
        match tokens[at] {
            Token::List(_) | Token::Quote => {
                balance += 1;
                marker = false;
            }
            Token::ListEnd | Token::QuoteEnd => {
                balance -= 1;
                marker = false;
            }
            Token::Ending(true) => {
                if item.is_some() && !marker && balance == 0 && blank.is_none() {
                    blank = Some(at);
                }
                marker = false;
            }
            Token::Space | Token::Item(_) => {}
            _ => marker = false,
        }

        let next = match tokens[at] {
            Token::Item(next) if balance == 0 => Some(next),
            _ => None,
        };
        if next.is_none() && balance != -1 {
            continue;
        }
        if let Some(item) = item {
            let mut tail = at;
            let mut line = None;
            while tail > 0 {
                tail -= 1;
                match tokens[tail] {
                    Token::Ending(_) => {
                        if line.is_some() {
                            spread[list] = true;
                        }
                        line = Some(tail);
                    }
                    Token::Space | Token::QuotePrefix => {}
                    _ => break,
                }
            }
            if blank.is_some_and(|blank| line.is_none_or(|line| blank < line)) {
                spread[item] = true;
            }
        }
        if next.is_some() {
            item = next;
            blank = None;
            marker = true;
        }
    }
}

/// Cindy: whether the list or the item that enters at each event is spread, as micromark infers it.
pub fn spreads(events: &[Event]) -> Vec<bool> {
    let tokens = tokens(events);
    let mut spread = vec![false; events.len()];
    let mut lists = Vec::new();
    for (at, token) in tokens.iter().enumerate() {
        match token {
            Token::List(_) => lists.push(at),
            Token::ListEnd => prepare(&tokens, lists.pop().unwrap(), at, &mut spread),
            _ => {}
        }
    }
    spread
}

/// Figure out the alignment of a GFM table.
pub fn gfm_table_align(events: &[Event], mut index: usize) -> Vec<AlignKind> {
    debug_assert!(
        matches!(events[index].name, Name::GfmTable),
        "expected table"
    );
    let mut in_delimiter_row = false;
    let mut align = vec![];

    while index < events.len() {
        let event = &events[index];

        if in_delimiter_row {
            if event.kind == Kind::Enter {
                // Start of alignment value: set a new column.
                if event.name == Name::GfmTableDelimiterCellValue {
                    align.push(if events[index + 1].name == Name::GfmTableDelimiterMarker {
                        AlignKind::Left
                    } else {
                        AlignKind::None
                    });
                }
            } else {
                // End of alignment value: change the column.
                if event.name == Name::GfmTableDelimiterCellValue {
                    if events[index - 1].name == Name::GfmTableDelimiterMarker {
                        let align_index = align.len() - 1;
                        align[align_index] = if align[align_index] == AlignKind::Left {
                            AlignKind::Center
                        } else {
                            AlignKind::Right
                        }
                    }
                }
                // Done!
                else if event.name == Name::GfmTableDelimiterRow {
                    break;
                }
            }
        } else if event.kind == Kind::Enter && event.name == Name::GfmTableDelimiterRow {
            in_delimiter_row = true;
        }

        index += 1;
    }

    align
}
