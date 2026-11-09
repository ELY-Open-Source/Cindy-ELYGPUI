//! `workRunGrouping.ts`'s `groupWorkRuns`: turns split by user rows and history gaps.

/// `HISTORY_GAP_SPLIT_MS`.
pub const HISTORY_GAP_SPLIT_MS: f64 = 30.0 * 60.0 * 1000.0;

/// `WorkRunGroupingAdapter`; items keep their identity unless they fold into a group.
pub trait Adapter {
    type Item: Clone;
    fn is_user_boundary(&self, item: &Self::Item) -> bool;
    fn is_continuation_boundary(&self, _item: &Self::Item) -> bool {
        false
    }
    fn is_answer(&self, item: &Self::Item) -> bool;
    fn can_prepend_to_answer(&self, _item: &Self::Item) -> bool {
        true
    }
    fn is_sealed_answer(&self, item: &Self::Item) -> bool;
    fn is_compact_boundary(&self, item: &Self::Item) -> bool;
    fn is_activity(&self, item: &Self::Item) -> bool;
    fn is_archivable(&self, item: &Self::Item) -> bool;
    fn start_timestamp(&self, item: &Self::Item) -> Option<f64>;
    fn end_timestamp(&self, item: &Self::Item) -> Option<f64>;
    fn boundary_timestamp(&self, item: Option<&Self::Item>) -> Option<f64>;
    fn user_boundary_end(&self, item: &Self::Item, previous: Option<f64>) -> Option<f64>;
    fn create_group(
        &self,
        run: Vec<Self::Item>,
        next: Option<&Self::Item>,
        streaming: bool,
        boundary: Option<f64>,
    ) -> Self::Item;
    fn create_completed_group(
        &self,
        run: Vec<Self::Item>,
        next: Option<&Self::Item>,
        boundary: Option<f64>,
    ) -> Self::Item;
    fn fold_answer_tail(&self) -> bool {
        true
    }
}

/// `groupWorkRuns`.
pub fn group<A: Adapter>(items: Vec<A::Item>, streaming: bool, adapter: &A) -> Vec<A::Item> {
    let mut out = Vec::new();
    let mut turn: Vec<A::Item> = Vec::new();
    let mut turn_start: Option<f64> = None;
    let mut previous_end: Option<f64> = None;
    let flush = |turn: &mut Vec<A::Item>, out: &mut Vec<A::Item>, start, tail, resumed| {
        let items = std::mem::take(turn);
        if items.is_empty() {
            return;
        }
        if tail && streaming {
            let completed = (0..items.len())
                .rev()
                .find(|&i| adapter.is_answer(&items[i]) && adapter.is_sealed_answer(&items[i]));
            let active_start = match completed {
                Some(i) => adapter.boundary_timestamp(Some(&items[i])),
                None => start,
            };
            let mut items = items;
            let rest = items.split_off(completed.map_or(0, |i| i + 1));
            if !items.is_empty() {
                let done = items.clone();
                out.extend(answered(items, start, adapter, false).unwrap_or(done));
            }
            out.extend(activity_runs(rest, active_start, true, adapter));
            return;
        }
        let fallback = items.clone();
        match answered(items, start, adapter, resumed) {
            Some(grouped) => out.extend(grouped),
            None => out.extend(activity_runs(fallback, start, false, adapter)),
        }
    };
    for item in items {
        let start = adapter.start_timestamp(&item);
        if let (Some(end), Some(start)) = (previous_end, start)
            && start - end > HISTORY_GAP_SPLIT_MS
        {
            flush(&mut turn, &mut out, turn_start, false, false);
            turn_start = None;
        }
        if adapter.is_user_boundary(&item) {
            let resumed = adapter.is_continuation_boundary(&item);
            flush(&mut turn, &mut out, turn_start, false, resumed);
            previous_end = adapter.user_boundary_end(&item, previous_end);
            turn_start = start;
            out.push(item);
            continue;
        }
        if let Some(end) = adapter.end_timestamp(&item) {
            previous_end = Some(previous_end.map_or(end, |p| p.max(end)));
        }
        turn.push(item);
    }
    flush(&mut turn, &mut out, turn_start, true, false);
    out
}

/// `groupActivityRuns`.
fn activity_runs<A: Adapter>(
    items: Vec<A::Item>,
    turn_start: Option<f64>,
    active: bool,
    adapter: &A,
) -> Vec<A::Item> {
    let last_completed = match active {
        true => items
            .iter()
            .rposition(|i| adapter.is_answer(i) || adapter.is_compact_boundary(i)),
        false => None,
    };
    let mut out = Vec::new();
    let mut run = Vec::new();
    let mut run_last = 0;
    let mut previous = turn_start;
    let streams = |index: usize| active && last_completed.is_none_or(|c| index > c);
    for (index, item) in items.into_iter().enumerate() {
        if adapter.is_activity(&item) {
            run.push(item);
            run_last = index;
            continue;
        }
        if !run.is_empty() {
            let group = adapter.create_group(
                std::mem::take(&mut run),
                Some(&item),
                streams(run_last),
                previous,
            );
            out.push(group);
        }
        previous = adapter.boundary_timestamp(Some(&item));
        out.push(item);
    }
    if !run.is_empty() {
        out.push(adapter.create_group(run, None, streams(run_last), previous));
    }
    out
}

/// The answers a turn keeps visible: seal-aware, else the last answer of an old history.
fn kept_answers<A: Adapter>(items: &[A::Item], adapter: &A, resumed: bool) -> Option<Vec<bool>> {
    let answer = |i: usize| adapter.is_answer(&items[i]);
    let last_answer = (0..items.len()).rposition(answer)?;
    let sealed: Vec<usize> = (0..items.len())
        .filter(|&i| answer(i) && adapter.is_sealed_answer(&items[i]))
        .collect();
    let mut kept = vec![false; items.len()];
    let prepend = |i: usize| answer(i) && adapter.can_prepend_to_answer(&items[i]);
    if let Some(&last_sealed) = sealed.last() {
        let mut segment_start = 0;
        for &sealed_index in &sealed {
            let last_activity = (segment_start..sealed_index)
                .rev()
                .find(|&i| adapter.is_activity(&items[i]));
            let floor = last_activity.map_or(0, |a| a + 1);
            let mut start = sealed_index;
            while start > floor && start > segment_start && prepend(start - 1) {
                start -= 1;
            }
            let keep = (!resumed && sealed_index == last_sealed)
                || (start..=sealed_index).any(|i| answer(i) && !adapter.is_archivable(&items[i]));
            if keep {
                (start..=sealed_index)
                    .filter(|&i| answer(i))
                    .for_each(|i| kept[i] = true);
            }
            segment_start = sealed_index + 1;
        }
    } else if !resumed {
        if (last_answer + 1..items.len()).any(|i| adapter.is_activity(&items[i])) {
            return None;
        }
        let mut start = last_answer;
        if let Some(activity) = (0..last_answer)
            .rev()
            .find(|&i| adapter.is_activity(&items[i]))
        {
            while start > activity + 1 && prepend(start - 1) {
                start -= 1;
            }
        }
        (start..=last_answer)
            .filter(|&i| answer(i))
            .for_each(|i| kept[i] = true);
    }
    Some(kept)
}

/// The run folded above the latest kept answer, which later tails may join.
struct BeforeAnswer<I> {
    index: usize,
    run: Vec<I>,
    next: I,
    boundary: Option<f64>,
    answer_ms: Option<f64>,
}

/// `groupAnsweredTurn`.
fn answered<A: Adapter>(
    items: Vec<A::Item>,
    turn_start: Option<f64>,
    adapter: &A,
    resumed: bool,
) -> Option<Vec<A::Item>> {
    let kept = kept_answers(&items, adapter, resumed)?;
    let mut out: Vec<A::Item> = Vec::new();
    let mut run: Vec<A::Item> = Vec::new();
    let mut previous = turn_start;
    let mut before: Option<BeforeAnswer<A::Item>> = None;
    // `foldTail`: a tail that started before the answer joins the run above it.
    let fold =
        |tail: &[A::Item], before: &mut Option<BeforeAnswer<A::Item>>, out: &mut Vec<A::Item>| {
            let Some(target) = before.as_ref() else {
                return false;
            };
            let Some(answer_ms) = target.answer_ms else {
                return false;
            };
            if !adapter.fold_answer_tail()
                || tail
                    .iter()
                    .any(|c| adapter.start_timestamp(c).is_none_or(|s| s > answer_ms))
            {
                return false;
            }
            let target = before.take().expect("checked above");
            let mut joined = target.run;
            joined.extend_from_slice(tail);
            out[target.index] =
                adapter.create_completed_group(joined, Some(&target.next), target.boundary);
            true
        };
    let flush = |run: &mut Vec<A::Item>,
                 next: Option<&A::Item>,
                 previous: Option<f64>,
                 before: &mut Option<BeforeAnswer<A::Item>>,
                 out: &mut Vec<A::Item>| {
        let flushed = std::mem::take(run);
        if !flushed.is_empty() && !fold(&flushed, before, out) {
            out.push(adapter.create_completed_group(flushed.clone(), next, previous));
        }
        flushed
    };
    for (index, item) in items.into_iter().enumerate() {
        if !kept[index] && adapter.is_archivable(&item) {
            run.push(item);
            continue;
        }
        let group_index = out.len();
        let flushed = flush(&mut run, Some(&item), previous, &mut before, &mut out);
        let grouped = out.len() > group_index;
        let boundary = adapter.boundary_timestamp(Some(&item));
        if !kept[index] {
            before = None;
        } else if grouped {
            before = Some(BeforeAnswer {
                index: group_index,
                run: flushed,
                next: item.clone(),
                boundary: previous,
                answer_ms: boundary,
            });
        } else if let Some(target) = before.as_mut() {
            target.answer_ms = boundary;
        }
        out.push(item);
        previous = boundary;
    }
    flush(&mut run, None, previous, &mut before, &mut out);
    Some(out)
}

#[cfg(test)]
mod tests;
