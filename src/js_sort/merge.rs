//! `MergeLow` and `MergeHigh`: two adjacent runs merged with galloping.

use super::{End, MIN_GALLOP_WINS, State, gallop_left, gallop_right};

impl<T: Copy> State<'_, T> {
    /// Run A copied out, merged from the left.
    pub(super) fn merge_low(
        &mut self,
        less: &mut impl FnMut(T, T) -> bool,
        (base_a, mut length_a): (usize, usize),
        (base_b, mut length_b): (usize, usize),
    ) {
        let temp = self.work[base_a..base_a + length_a].to_vec();
        let work = &mut *self.work;
        let (mut dest, mut cursor_temp, mut cursor_b) = (base_a, 0, base_b);
        work[dest] = work[cursor_b];
        (dest, cursor_b) = (dest + 1, cursor_b + 1);
        let end = 'merge: {
            length_b -= 1;
            if length_b == 0 {
                break 'merge End::Succeed;
            }
            if length_a == 1 {
                break 'merge End::CopyRest;
            }
            let mut min_gallop = self.min_gallop;
            loop {
                let (mut wins_a, mut wins_b) = (0, 0);
                loop {
                    if less(work[cursor_b], temp[cursor_temp]) {
                        work[dest] = work[cursor_b];
                        (dest, cursor_b) = (dest + 1, cursor_b + 1);
                        (wins_b, wins_a, length_b) = (wins_b + 1, 0, length_b - 1);
                        if length_b == 0 {
                            break 'merge End::Succeed;
                        }
                        if wins_b >= min_gallop {
                            break;
                        }
                    } else {
                        work[dest] = temp[cursor_temp];
                        (dest, cursor_temp) = (dest + 1, cursor_temp + 1);
                        (wins_a, wins_b, length_a) = (wins_a + 1, 0, length_a - 1);
                        if length_a == 1 {
                            break 'merge End::CopyRest;
                        }
                        if wins_a >= min_gallop {
                            break;
                        }
                    }
                }
                min_gallop += 1;
                let mut first = true;
                while wins_a >= MIN_GALLOP_WINS || wins_b >= MIN_GALLOP_WINS || first {
                    first = false;
                    min_gallop = (min_gallop - 1).max(1);
                    self.min_gallop = min_gallop;
                    wins_a = gallop_right(&temp, less, work[cursor_b], cursor_temp, length_a, 0);
                    if wins_a > 0 {
                        work[dest..dest + wins_a]
                            .copy_from_slice(&temp[cursor_temp..cursor_temp + wins_a]);
                        (dest, cursor_temp, length_a) =
                            (dest + wins_a, cursor_temp + wins_a, length_a - wins_a);
                        if length_a == 1 {
                            break 'merge End::CopyRest;
                        }
                        if length_a == 0 {
                            break 'merge End::Succeed;
                        }
                    }
                    work[dest] = work[cursor_b];
                    (dest, cursor_b, length_b) = (dest + 1, cursor_b + 1, length_b - 1);
                    if length_b == 0 {
                        break 'merge End::Succeed;
                    }
                    wins_b = gallop_left(work, less, temp[cursor_temp], cursor_b, length_b, 0);
                    if wins_b > 0 {
                        work.copy_within(cursor_b..cursor_b + wins_b, dest);
                        (dest, cursor_b, length_b) =
                            (dest + wins_b, cursor_b + wins_b, length_b - wins_b);
                        if length_b == 0 {
                            break 'merge End::Succeed;
                        }
                    }
                    work[dest] = temp[cursor_temp];
                    (dest, cursor_temp, length_a) = (dest + 1, cursor_temp + 1, length_a - 1);
                    if length_a == 1 {
                        break 'merge End::CopyRest;
                    }
                }
                min_gallop += 1;
                self.min_gallop = min_gallop;
            }
        };
        match end {
            End::Succeed => work[dest..dest + length_a]
                .copy_from_slice(&temp[cursor_temp..cursor_temp + length_a]),
            End::CopyRest => {
                work.copy_within(cursor_b..cursor_b + length_b, dest);
                work[dest + length_b] = temp[cursor_temp];
            }
        }
    }

    /// Run B copied out, merged from the right.
    pub(super) fn merge_high(
        &mut self,
        less: &mut impl FnMut(T, T) -> bool,
        (base_a, mut length_a): (usize, usize),
        (base_b, mut length_b): (usize, usize),
    ) {
        let temp = self.work[base_b..base_b + length_b].to_vec();
        let work = &mut *self.work;
        let u = |i: isize| i as usize;
        let mut dest = (base_b + length_b - 1) as isize;
        let mut cursor_temp = length_b as isize - 1;
        let mut cursor_a = (base_a + length_a - 1) as isize;
        work[u(dest)] = work[u(cursor_a)];
        (dest, cursor_a) = (dest - 1, cursor_a - 1);
        let end = 'merge: {
            length_a -= 1;
            if length_a == 0 {
                break 'merge End::Succeed;
            }
            if length_b == 1 {
                break 'merge End::CopyRest;
            }
            let mut min_gallop = self.min_gallop;
            loop {
                let (mut wins_a, mut wins_b) = (0, 0);
                loop {
                    if less(temp[u(cursor_temp)], work[u(cursor_a)]) {
                        work[u(dest)] = work[u(cursor_a)];
                        (dest, cursor_a) = (dest - 1, cursor_a - 1);
                        (wins_a, wins_b, length_a) = (wins_a + 1, 0, length_a - 1);
                        if length_a == 0 {
                            break 'merge End::Succeed;
                        }
                        if wins_a >= min_gallop {
                            break;
                        }
                    } else {
                        work[u(dest)] = temp[u(cursor_temp)];
                        (dest, cursor_temp) = (dest - 1, cursor_temp - 1);
                        (wins_b, wins_a, length_b) = (wins_b + 1, 0, length_b - 1);
                        if length_b == 1 {
                            break 'merge End::CopyRest;
                        }
                        if wins_b >= min_gallop {
                            break;
                        }
                    }
                }
                min_gallop += 1;
                let mut first = true;
                while wins_a >= MIN_GALLOP_WINS || wins_b >= MIN_GALLOP_WINS || first {
                    first = false;
                    min_gallop = (min_gallop - 1).max(1);
                    self.min_gallop = min_gallop;
                    let key = temp[u(cursor_temp)];
                    let k = gallop_right(work, less, key, base_a, length_a, length_a - 1);
                    wins_a = length_a - k;
                    if wins_a > 0 {
                        let n = wins_a as isize;
                        (dest, cursor_a) = (dest - n, cursor_a - n);
                        work.copy_within(u(cursor_a + 1)..u(cursor_a + 1) + wins_a, u(dest + 1));
                        length_a -= wins_a;
                        if length_a == 0 {
                            break 'merge End::Succeed;
                        }
                    }
                    work[u(dest)] = temp[u(cursor_temp)];
                    (dest, cursor_temp, length_b) = (dest - 1, cursor_temp - 1, length_b - 1);
                    if length_b == 1 {
                        break 'merge End::CopyRest;
                    }
                    let key = work[u(cursor_a)];
                    let k = gallop_left(&temp, less, key, 0, length_b, length_b - 1);
                    wins_b = length_b - k;
                    if wins_b > 0 {
                        let n = wins_b as isize;
                        (dest, cursor_temp) = (dest - n, cursor_temp - n);
                        let from = u(cursor_temp + 1);
                        work[u(dest + 1)..u(dest + 1) + wins_b]
                            .copy_from_slice(&temp[from..from + wins_b]);
                        length_b -= wins_b;
                        if length_b == 1 {
                            break 'merge End::CopyRest;
                        }
                        if length_b == 0 {
                            break 'merge End::Succeed;
                        }
                    }
                    work[u(dest)] = work[u(cursor_a)];
                    (dest, cursor_a, length_a) = (dest - 1, cursor_a - 1, length_a - 1);
                    if length_a == 0 {
                        break 'merge End::Succeed;
                    }
                }
                min_gallop += 1;
                self.min_gallop = min_gallop;
            }
        };
        match end {
            End::Succeed => {
                let at = u(dest - (length_b as isize - 1));
                work[at..at + length_b].copy_from_slice(&temp[..length_b]);
            }
            End::CopyRest => {
                let n = length_a as isize;
                (dest, cursor_a) = (dest - n, cursor_a - n);
                work.copy_within(u(cursor_a + 1)..u(cursor_a + 1) + length_a, u(dest + 1));
                work[u(dest)] = temp[u(cursor_temp)];
            }
        }
    }
}
