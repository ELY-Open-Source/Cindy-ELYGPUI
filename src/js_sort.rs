//! V8's `Array.prototype.sort` (`array-sort.tq`), so a non-total order sorts as Cindy's does.

const MIN_GALLOP_WINS: usize = 7;

enum End {
    Succeed,
    CopyRest,
}

struct State<'a, T> {
    work: &'a mut [T],
    runs: Vec<(usize, usize)>,
    min_gallop: usize,
}

/// `ArrayTimSort`; `less(a, b)` is `comparefn(a, b) < 0`.
pub fn sort<T: Copy>(items: &mut [T], mut less: impl FnMut(T, T) -> bool) {
    let length = items.len();
    if length < 8 {
        binary_insertion_sort(items, &mut less, 0, 0, length);
        return;
    }
    let mut state = State {
        work: items,
        runs: Vec::new(),
        min_gallop: MIN_GALLOP_WINS,
    };
    let min_run = min_run_length(length);
    let (mut low, mut remaining) = (0, length);
    while remaining != 0 {
        let mut run = count_and_make_run(state.work, &mut less, low, low + remaining);
        if run < min_run {
            let forced = min_run.min(remaining);
            binary_insertion_sort(state.work, &mut less, low, low + run, low + forced);
            run = forced;
        }
        state.runs.push((low, run));
        state.merge_collapse(&mut less);
        low += run;
        remaining -= run;
    }
    while state.runs.len() > 1 {
        let mut n = state.runs.len() - 2;
        if n > 0 && state.runs[n - 1].1 < state.runs[n + 1].1 {
            n -= 1;
        }
        state.merge_at(n, &mut less);
    }
}

fn min_run_length(mut n: usize) -> usize {
    let mut r = 0;
    while n >= 64 {
        r |= n & 1;
        n >>= 1;
    }
    n + r
}

fn binary_insertion_sort<T: Copy>(
    a: &mut [T],
    less: &mut impl FnMut(T, T) -> bool,
    low: usize,
    start: usize,
    high: usize,
) {
    let first = if low == start { start + 1 } else { start };
    for start in first..high {
        let pivot = a[start];
        let (mut left, mut right) = (low, start);
        while left < right {
            let mid = left + ((right - left) >> 1);
            if less(pivot, a[mid]) {
                right = mid;
            } else {
                left = mid + 1;
            }
        }
        a.copy_within(left..start, left + 1);
        a[left] = pivot;
    }
}

/// `CountAndMakeRun`: the run at `low`, a strictly descending one reversed.
fn count_and_make_run<T: Copy>(
    a: &mut [T],
    less: &mut impl FnMut(T, T) -> bool,
    low: usize,
    high: usize,
) -> usize {
    if low + 1 == high {
        return 1;
    }
    let descending = less(a[low + 1], a[low]);
    let (mut run, mut previous) = (2, a[low + 1]);
    for &current in &a[low + 2..high] {
        if less(current, previous) != descending {
            break;
        }
        previous = current;
        run += 1;
    }
    if descending {
        a[low..low + run].reverse();
    }
    run
}

/// `GallopLeft`: where `key` goes in `a[base..base + length]`, left of equal ones.
fn gallop_left<T: Copy>(
    a: &[T],
    less: &mut impl FnMut(T, T) -> bool,
    key: T,
    base: usize,
    length: usize,
    hint: usize,
) -> usize {
    let at = |i: isize| a[base + i as usize];
    let (length, hint) = (length as isize, hint as isize);
    let (mut last, mut offset) = (0, 1);
    if less(at(hint), key) {
        let max = length - hint;
        while offset < max && less(at(hint + offset), key) {
            last = offset;
            offset = (offset << 1) + 1;
        }
        offset = offset.min(max);
        (last, offset) = (last + hint, offset + hint);
    } else {
        let max = hint + 1;
        while offset < max && !less(at(hint - offset), key) {
            last = offset;
            offset = (offset << 1) + 1;
        }
        offset = offset.min(max);
        (last, offset) = (hint - offset, hint - last);
    }
    last += 1;
    while last < offset {
        let m = last + ((offset - last) >> 1);
        if less(at(m), key) {
            last = m + 1;
        } else {
            offset = m;
        }
    }
    offset as usize
}

/// `GallopRight`: where `key` goes in `a[base..base + length]`, right of equal ones.
fn gallop_right<T: Copy>(
    a: &[T],
    less: &mut impl FnMut(T, T) -> bool,
    key: T,
    base: usize,
    length: usize,
    hint: usize,
) -> usize {
    let at = |i: isize| a[base + i as usize];
    let (length, hint) = (length as isize, hint as isize);
    let (mut last, mut offset) = (0, 1);
    if less(key, at(hint)) {
        let max = hint + 1;
        while offset < max && less(key, at(hint - offset)) {
            last = offset;
            offset = (offset << 1) + 1;
        }
        offset = offset.min(max);
        (last, offset) = (hint - offset, hint - last);
    } else {
        let max = length - hint;
        while offset < max && !less(key, at(hint + offset)) {
            last = offset;
            offset = (offset << 1) + 1;
        }
        offset = offset.min(max);
        (last, offset) = (last + hint, offset + hint);
    }
    last += 1;
    while last < offset {
        let m = last + ((offset - last) >> 1);
        if less(key, at(m)) {
            offset = m;
        } else {
            last = m + 1;
        }
    }
    offset as usize
}

impl<T: Copy> State<'_, T> {
    /// A run stack whose lengths grow at least as Fibonacci numbers do.
    fn established(&self, n: usize) -> bool {
        n < 2 || self.runs[n - 2].1 > self.runs[n - 1].1 + self.runs[n].1
    }

    fn merge_collapse(&mut self, less: &mut impl FnMut(T, T) -> bool) {
        while self.runs.len() > 1 {
            let mut n = self.runs.len() - 2;
            if !self.established(n + 1) || !self.established(n) {
                if self.runs[n - 1].1 < self.runs[n + 1].1 {
                    n -= 1;
                }
                self.merge_at(n, less);
            } else if self.runs[n].1 <= self.runs[n + 1].1 {
                self.merge_at(n, less);
            } else {
                break;
            }
        }
    }

    fn merge_at(&mut self, i: usize, less: &mut impl FnMut(T, T) -> bool) {
        let (mut base_a, mut length_a) = self.runs[i];
        let (base_b, mut length_b) = self.runs[i + 1];
        self.runs[i].1 = length_a + length_b;
        if i + 3 == self.runs.len() {
            self.runs[i + 1] = self.runs[i + 2];
        }
        self.runs.pop();
        let k = gallop_right(self.work, less, self.work[base_b], base_a, length_a, 0);
        base_a += k;
        length_a -= k;
        if length_a == 0 {
            return;
        }
        let key = self.work[base_a + length_a - 1];
        length_b = gallop_left(self.work, less, key, base_b, length_b, length_b - 1);
        if length_b == 0 {
            return;
        }
        if length_a <= length_b {
            self.merge_low(less, (base_a, length_a), (base_b, length_b));
        } else {
            self.merge_high(less, (base_a, length_a), (base_b, length_b));
        }
    }
}

mod merge;

#[cfg(test)]
mod tests;
