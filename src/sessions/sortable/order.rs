//! How a drag rearranges the shown columns, and the order a drop saves.

/// What the pointer is over.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Entry(String),
    /// A column's empty tail, below its last entry (`_ghostIsLast`).
    Tail(usize),
}

/// `DraggableCardColumns`' buckets: entry `i` in column `i % n`.
pub fn buckets<T>(items: Vec<T>, n: usize) -> Vec<Vec<T>> {
    let mut out: Vec<Vec<T>> = (0..n.max(1)).map(|_| Vec::new()).collect();
    let n = out.len();
    for (i, item) in items.into_iter().enumerate() {
        out[i % n].push(item);
    }
    out
}

pub fn locate(columns: &[Vec<String>], id: &str) -> Option<(usize, usize)> {
    columns
        .iter()
        .enumerate()
        .find_map(|(c, col)| col.iter().position(|e| e == id).map(|r| (c, r)))
}

/// One SortableJS move with `swapThreshold: 1`: in its own column the entry takes the target's
/// slot; in another it goes after the target when it sat higher, else before
/// (`_getInsertDirection`). Returns whether anything moved.
pub fn step(columns: &mut [Vec<String>], dragged: &str, target: &Target) -> bool {
    let Some((c, r)) = locate(columns, dragged) else {
        return false;
    };
    let (tc, at) = match target {
        Target::Entry(id) if id == dragged => return false,
        Target::Entry(id) => {
            let Some((tc, tr)) = locate(columns, id) else {
                return false;
            };
            (tc, if tc == c || r >= tr { tr } else { tr + 1 })
        }
        Target::Tail(tc) if *tc >= columns.len() => return false,
        Target::Tail(tc) if *tc == c && r + 1 == columns[c].len() => return false,
        Target::Tail(tc) => (*tc, columns[*tc].len()),
    };
    let id = columns[c].remove(r);
    let at = at.min(columns[tc].len());
    columns[tc].insert(at, id);
    true
}

/// `reorderByDropSlot`: the moved entry goes to `row * n + column` among the others.
fn by_drop_slot(
    current: &[String],
    moved: &str,
    column: usize,
    row: usize,
    n: usize,
) -> Vec<String> {
    let mut rest: Vec<String> = current.iter().filter(|e| *e != moved).cloned().collect();
    let at = (row * n + column).min(rest.len());
    rest.insert(at, moved.to_owned());
    rest
}

/// The order a drop saves: one column as it stands, several by the dropped slot.
pub fn dropped(start: &[String], columns: &[Vec<String>], dragged: &str) -> Vec<String> {
    match (columns, locate(columns, dragged)) {
        ([only], _) => only.clone(),
        (_, Some((c, r))) => by_drop_slot(start, dragged, c, r, columns.len()),
        (_, None) => start.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::{Target, buckets, by_drop_slot, dropped, step};

    fn cols(spec: &[&[&str]]) -> Vec<Vec<String>> {
        spec.iter()
            .map(|c| c.iter().map(|s| s.to_string()).collect())
            .collect()
    }

    fn own(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn drop_slots_follow_reorder_by_drop_slot() {
        let order = own(&["a", "b", "c", "d"]);
        assert_eq!(by_drop_slot(&order, "c", 1, 1, 2), ["a", "b", "d", "c"]);
        assert_eq!(by_drop_slot(&order, "a", 1, 1, 2), ["b", "c", "d", "a"]);
        assert_eq!(by_drop_slot(&order, "c", 1, 0, 2), ["a", "c", "b", "d"]);
        assert_eq!(buckets(order, 3), cols(&[&["a", "d"], &["b"], &["c"]]));
    }

    #[test]
    fn a_move_in_one_column_takes_the_slot() {
        let mut c = cols(&[&["a", "b", "c"]]);
        assert!(step(&mut c, "a", &Target::Entry("c".into())));
        assert_eq!(c, cols(&[&["b", "c", "a"]]));
        assert!(!step(&mut c, "a", &Target::Tail(0)), "already last");
        assert!(step(&mut c, "b", &Target::Tail(0)));
        assert_eq!(c, cols(&[&["c", "a", "b"]]));
    }

    #[test]
    fn a_move_across_columns_goes_by_rows() {
        let mut c = cols(&[&["a", "c"], &["b", "d"]]);
        assert!(
            step(&mut c, "a", &Target::Entry("d".into())),
            "higher, so after"
        );
        assert_eq!(c, cols(&[&["c"], &["b", "d", "a"]]));
        assert!(
            step(&mut c, "a", &Target::Entry("c".into())),
            "lower, so before"
        );
        assert_eq!(c, cols(&[&["a", "c"], &["b", "d"]]));
        assert!(step(&mut c, "c", &Target::Tail(1)));
        assert_eq!(c, cols(&[&["a"], &["b", "d", "c"]]));
        assert!(!step(&mut c, "c", &Target::Entry("c".into())));
    }

    #[test]
    fn a_drop_saves_by_column_count() {
        let start = own(&["a", "b", "c", "d"]);
        assert_eq!(
            dropped(&start, &cols(&[&["b", "a", "c", "d"]]), "a"),
            ["b", "a", "c", "d"]
        );
        let moved = cols(&[&["a"], &["b", "d", "c"]]);
        assert_eq!(
            dropped(&start, &moved, "c"),
            ["a", "b", "d", "c"].map(String::from).to_vec()
        );
    }
}
