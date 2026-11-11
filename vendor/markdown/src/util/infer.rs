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

/// Cindy: mdast-util-from-markdown's `listSpread`: two line endings between two items.
pub fn list_spread(events: &[Event], start: usize) -> bool {
    let mut balance = 0;
    let mut index = start;

    while index < events.len() {
        let event = &events[index];

        if event.kind == Kind::Enter {
            balance += 1;

            if balance == 2 && event.name == Name::ListItem && index > start + 1 {
                let mut tail = index;
                let mut lines = 0;
                let mut quotes = 0;
                let mut depth = None;

                while tail > start {
                    tail -= 1;
                    let event = &events[tail];
                    match event.name {
                        Name::LineEnding | Name::BlankLineEnding => {
                            if event.kind == Kind::Enter {
                                lines += 1;
                                depth.get_or_insert(quotes);
                                quotes = 0;
                            }
                        }
                        // Cindy: a quote that closed here ends before its last line's ending.
                        Name::BlockQuotePrefix => {
                            if event.kind == Kind::Enter {
                                quotes += 1;
                                if depth.is_some_and(|depth| quotes > depth) {
                                    break;
                                }
                            }
                        }
                        Name::SpaceOrTab | Name::BlockQuoteMarker => {}
                        Name::ListItem
                        | Name::ListOrdered
                        | Name::ListUnordered
                        | Name::BlockQuote
                            if event.kind == Kind::Exit => {}
                        _ => break,
                    }
                }

                if lines > 1 {
                    return true;
                }
            }
        } else {
            balance -= 1;

            if balance == 0 {
                break;
            }
        }

        index += 1;
    }

    false
}

pub fn list_item_loose(events: &[Event], start: usize) -> bool {
    debug_assert!(
        matches!(events[start].name, Name::ListItem),
        "expected list item"
    );
    let mut balance = 0;
    let mut index = start;
    let mut blank = None;

    while index < events.len() {
        let event = &events[index];

        if event.kind == Kind::Enter {
            balance += 1;
        } else {
            balance -= 1;

            if balance == 1 && event.name == Name::BlankLineEnding && blank.is_none() {
                // Blank line directly after a prefix:
                //
                // ```markdown
                // > | -␊
                //      ^
                //   |   a
                // ```
                let mut before = index - 2;

                if events[before].name == Name::SpaceOrTab {
                    before -= 2;
                }

                // Cindy: a blank line inside a block quote belongs to the quote.
                let mut quote = index - 2;
                let mut closed = false;
                while events[quote].name == Name::SpaceOrTab
                    || (events[quote].name == Name::BlockQuote
                        && events[quote].kind == Kind::Exit)
                {
                    closed = closed || events[quote].name == Name::BlockQuote;
                    quote -= 1;
                }

                // Cindy: so does the blank line that opens a nested empty item.
                let mut nested = index - 2;
                let mut ended = false;
                while events[nested].name == Name::SpaceOrTab
                    || (events[nested].kind == Kind::Exit
                        && matches!(
                            events[nested].name,
                            Name::ListItem
                                | Name::ListOrdered
                                | Name::ListUnordered
                                | Name::BlockQuote
                        ))
                {
                    ended = ended || events[nested].name != Name::SpaceOrTab;
                    nested -= 1;
                }

                if events[before].name != Name::ListItemPrefix
                    && !(closed && events[quote].name == Name::BlockQuotePrefix)
                    && !(ended && events[nested].name == Name::ListItemPrefix)
                {
                    blank = Some(index - 1);
                }
            }

            // Done.
            if balance == 0 && event.name == Name::ListItem {
                break;
            }
        }

        index += 1;
    }

    // Cindy: micromark's `prepareList`: the blank lines that end an item do not spread it.
    let Some(blank) = blank else { return false };
    let mut line = None;

    while index > start {
        index -= 1;
        match events[index].name {
            Name::LineEnding | Name::BlankLineEnding => {
                if events[index].kind == Kind::Enter {
                    line = Some(index);
                }
            }
            Name::SpaceOrTab | Name::BlockQuotePrefix | Name::BlockQuoteMarker => {}
            _ => break,
        }
    }

    line.is_none_or(|line| blank < line)
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
