//! Sequence alignment for the diff engines (PLAN phase 5, section 6, WP12).
//!
//! [`matches`] aligns two sequences of keys the way `git diff --patience` does: the common prefix and suffix are taken,
//! then the keys that occur exactly once on each side are anchors, the longest increasing run of them is kept, and the
//! gaps between anchors are aligned the same way. A gap with no unique key falls back to Myers' O(ND) greedy diff,
//! capped at [`MAX_D`] edits (beyond that the gap is left unmatched: a rewrite). Unique anchors keep blank lines and
//! repeated boilerplate from becoming false matches, and a moved unique item falls out of the increasing run, so the
//! caller sees it as one deletion plus one insertion with equal keys (move detection pairs those).
//!
//! [`pair`] then matches the items left in one gap by similarity (edited paragraphs, edited rows).
use std::collections::HashMap;
use std::hash::Hash;
use std::ops::Range;

/// Most edits the Myers fallback explores in one gap. Its trace holds about `D^2` offsets (16 MB at the cap).
pub const MAX_D: usize = 2048;

/// Matched index pairs `(i, j)` with `a[i] == b[j]`, strictly increasing in both.
pub fn matches<T: Eq + Hash>(a: &[T], b: &[T]) -> Vec<(usize, usize)> {
    enum Task {
        Range(Range<usize>, Range<usize>),
        Emit(usize, usize),
    }
    let mut out = Vec::new();
    // An explicit stack, not recursion: a sheet's 20,000 rows could nest that deep.
    let mut todo = vec![Task::Range(0..a.len(), 0..b.len())];
    while let Some(t) = todo.pop() {
        let (mut ra, mut rb) = match t {
            Task::Emit(i, j) => {
                out.push((i, j));
                continue;
            }
            Task::Range(ra, rb) => (ra, rb),
        };
        while ra.start < ra.end && rb.start < rb.end && a[ra.start] == b[rb.start] {
            out.push((ra.start, rb.start));
            ra.start += 1;
            rb.start += 1;
        }
        let mut tail = Vec::new();
        while ra.start < ra.end && rb.start < rb.end && a[ra.end - 1] == b[rb.end - 1] {
            ra.end -= 1;
            rb.end -= 1;
            tail.push(Task::Emit(ra.end, rb.end));
        }
        // the suffix is emitted after the middle: pushed first (stack order), nearest last
        todo.extend(tail);
        if ra.is_empty() || rb.is_empty() {
            continue;
        }
        let anchors = unique_anchors(a, b, ra.clone(), rb.clone());
        if anchors.is_empty() {
            if let Some(m) = myers(&a[ra.clone()], &b[rb.clone()], MAX_D) {
                todo.extend(
                    m.into_iter()
                        .rev()
                        .map(|(i, j)| Task::Emit(ra.start + i, rb.start + j)),
                );
            }
            continue;
        }
        // gaps between anchors, pushed in reverse so they come out in order
        let mut next = (ra.end, rb.end);
        for &(i, j) in anchors.iter().rev() {
            todo.push(Task::Range(i + 1..next.0, j + 1..next.1));
            todo.push(Task::Emit(i, j));
            next = (i, j);
        }
        todo.push(Task::Range(ra.start..next.0, rb.start..next.1));
    }
    out
}

/// The keys unique on both sides of the ranges, as `(i, j)`, reduced to the longest run increasing in both.
fn unique_anchors<T: Eq + Hash>(
    a: &[T],
    b: &[T],
    ra: Range<usize>,
    rb: Range<usize>,
) -> Vec<(usize, usize)> {
    let mut seen: HashMap<&T, (u32, u32, usize, usize)> = HashMap::new();
    for i in ra {
        let e = seen.entry(&a[i]).or_insert((0, 0, i, 0));
        e.0 += 1;
    }
    for j in rb {
        if let Some(e) = seen.get_mut(&b[j]) {
            e.1 += 1;
            e.3 = j;
        }
    }
    let mut cand: Vec<(usize, usize)> = seen
        .into_values()
        .filter(|e| e.0 == 1 && e.1 == 1)
        .map(|e| (e.2, e.3))
        .collect();
    cand.sort_unstable();
    lis(&cand)
}

/// Longest subsequence of `c` (sorted by `.0`) increasing in `.1`: patience sorting with predecessor links.
fn lis(c: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut tails: Vec<usize> = Vec::new(); // index into c of the smallest tail of each length
    let mut prev = vec![usize::MAX; c.len()];
    for (k, &(_, j)) in c.iter().enumerate() {
        let pos = tails.partition_point(|&t| c[t].1 < j);
        if pos > 0 {
            prev[k] = tails[pos - 1];
        }
        if pos == tails.len() {
            tails.push(k);
        } else {
            tails[pos] = k;
        }
    }
    let mut out = Vec::with_capacity(tails.len());
    let mut k = tails.last().copied().unwrap_or(usize::MAX);
    while k != usize::MAX {
        out.push(c[k]);
        k = prev[k];
    }
    out.reverse();
    out
}

/// Myers' greedy shortest edit script; the matched pairs, or `None` when more than `max_d` edits are needed.
fn myers<T: Eq>(a: &[T], b: &[T], max_d: usize) -> Option<Vec<(usize, usize)>> {
    let (n, m) = (a.len() as isize, b.len() as isize);
    let max = (n + m) as usize;
    let off = max as isize + 1;
    let mut v = vec![0isize; 2 * max + 3];
    // trace[d] = v over the diagonals -d..=d after step d
    let mut trace: Vec<Vec<isize>> = Vec::new();
    for d in 0..=(max.min(max_d) as isize) {
        let mut done = false;
        let mut k = -d;
        while k <= d {
            let down = k == -d || (k != d && v[(off + k - 1) as usize] < v[(off + k + 1) as usize]);
            let mut x = if down {
                v[(off + k + 1) as usize]
            } else {
                v[(off + k - 1) as usize] + 1
            };
            let mut y = x - k;
            while x < n && y < m && a[x as usize] == b[y as usize] {
                x += 1;
                y += 1;
            }
            v[(off + k) as usize] = x;
            if x >= n && y >= m {
                done = true;
                break;
            }
            k += 2;
        }
        trace.push(v[(off - d) as usize..=(off + d) as usize].to_vec());
        if done {
            return Some(backtrack(&trace, n, m));
        }
    }
    None
}

fn backtrack(trace: &[Vec<isize>], n: isize, m: isize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let (mut x, mut y) = (n, m);
    for d in (0..trace.len() as isize).rev() {
        let k = x - y;
        let (px, py) = if d == 0 {
            (0, 0)
        } else {
            let pv = &trace[(d - 1) as usize];
            let at = |k: isize| pv[(k + d - 1) as usize];
            let down = k == -d || (k != d && at(k - 1) < at(k + 1));
            let pk = if down { k + 1 } else { k - 1 };
            let px = at(pk);
            let (sx, sy) = if down {
                (px, px - pk + 1)
            } else {
                (px + 1, px - pk)
            };
            // the diagonal run after this step's edit
            while x > sx && y > sy {
                x -= 1;
                y -= 1;
                out.push((x as usize, y as usize));
            }
            (px, px - pk)
        };
        if d == 0 {
            while x > px && y > py {
                x -= 1;
                y -= 1;
                out.push((x as usize, y as usize));
            }
        }
        x = px;
        y = py;
    }
    out.reverse();
    out
}

/// The gaps between consecutive matches (and before the first and after the last) as `(a range, b range)`;
/// only the non-empty ones.
pub fn gaps(matches: &[(usize, usize)], na: usize, nb: usize) -> Vec<(Range<usize>, Range<usize>)> {
    let mut out = Vec::new();
    let mut at = (0, 0);
    for &(i, j) in matches.iter().chain(std::iter::once(&(na, nb))) {
        if i > at.0 || j > at.1 {
            out.push((at.0..i, at.1..j));
        }
        at = (i + 1, j + 1);
    }
    out
}

/// A monotone pairing of `n` left and `m` right items with the largest total `sim`, keeping only pairs whose
/// similarity is at least `min`. O(n·m) time and memory: callers bound `n * m`.
pub fn pair(
    n: usize,
    m: usize,
    min: f64,
    mut sim: impl FnMut(usize, usize) -> f64,
) -> Vec<(usize, usize)> {
    let w = m + 1;
    let mut score = vec![0f64; (n + 1) * w];
    let mut s = vec![0f64; n * m];
    for i in 1..=n {
        for j in 1..=m {
            let x = sim(i - 1, j - 1);
            s[(i - 1) * m + j - 1] = x;
            let diag = if x >= min {
                score[(i - 1) * w + j - 1] + x
            } else {
                f64::MIN
            };
            score[i * w + j] = score[(i - 1) * w + j].max(score[i * w + j - 1]).max(diag);
        }
    }
    let mut out = Vec::new();
    let (mut i, mut j) = (n, m);
    while i > 0 && j > 0 {
        let x = s[(i - 1) * m + j - 1];
        if x >= min && score[i * w + j] == score[(i - 1) * w + j - 1] + x {
            out.push((i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if score[i * w + j] == score[(i - 1) * w + j] {
            i -= 1;
        } else {
            j -= 1;
        }
    }
    out.reverse();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(a: &str, b: &str) -> Vec<(usize, usize)> {
        let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
        let m = matches(&a, &b);
        for w in m.windows(2) {
            assert!(w[0].0 < w[1].0 && w[0].1 < w[1].1, "{m:?}");
        }
        for &(i, j) in &m {
            assert_eq!(a[i], b[j]);
        }
        m
    }

    #[test]
    fn identical_prefix_suffix_and_empty() {
        assert_eq!(check("abc", "abc").len(), 3);
        assert!(check("", "abc").is_empty());
        assert_eq!(check("abXc", "abYc"), vec![(0, 0), (1, 1), (3, 3)]);
    }

    #[test]
    fn myers_is_minimal_without_unique_keys() {
        // no key unique on both sides: the Myers fallback; LCS of these is 4
        assert_eq!(check("aabbaabb", "abababab").len(), 6);
        let m = myers(&[1, 1, 2, 2], &[2, 2, 1, 1], MAX_D).unwrap_or_default();
        assert_eq!(m.len(), 2);
        assert!(myers(&[1; 10], &[2; 10], 5).is_none());
    }

    #[test]
    fn a_moved_unique_item_is_unmatched() {
        // "x" moved from the front to the back: the rest stays matched, x on one side only
        let m = check("xabcd", "abcdx");
        assert_eq!(m, vec![(1, 0), (2, 1), (3, 2), (4, 3)]);
        assert_eq!(gaps(&m, 5, 5), vec![(0..1, 0..0), (5..5, 4..5)]);
    }

    #[test]
    fn deep_inputs_do_not_overflow_the_stack() {
        let a: Vec<u32> = (0..50_000).collect();
        let b: Vec<u32> = (0..50_000).rev().collect();
        assert_eq!(matches(&a, &b).len(), 1);
    }

    #[test]
    fn lcs_matches_brute_force_on_small_inputs() {
        // compare with a DP LCS length on a deterministic pseudo-random set; patience is not always minimal, but
        // without unique anchors (alphabet 2, long strings) Myers is
        let mut seed = 7u32;
        let mut rnd = |k: u32| {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            (seed >> 16) % k
        };
        for _ in 0..200 {
            let a: Vec<u32> = (0..rnd(12)).map(|_| rnd(3)).collect();
            let b: Vec<u32> = (0..rnd(12)).map(|_| rnd(3)).collect();
            let got = matches(&a, &b).len();
            let mut dp = vec![vec![0usize; b.len() + 1]; a.len() + 1];
            for i in 1..=a.len() {
                for j in 1..=b.len() {
                    dp[i][j] = if a[i - 1] == b[j - 1] {
                        dp[i - 1][j - 1] + 1
                    } else {
                        dp[i - 1][j].max(dp[i][j - 1])
                    };
                }
            }
            assert!(got <= dp[a.len()][b.len()]);
            assert!(
                myers(&a, &b, MAX_D).map(|m| m.len()) == Some(dp[a.len()][b.len()]),
                "{a:?} {b:?}"
            );
        }
    }

    #[test]
    fn pair_prefers_the_similar_items_in_order() {
        let sims = [[0.9, 0.1, 0.0], [0.0, 0.2, 0.8]];
        assert_eq!(pair(2, 3, 0.5, |i, j| sims[i][j]), vec![(0, 0), (1, 2)]);
        assert!(pair(2, 3, 0.95, |i, j| sims[i][j]).is_empty());
    }
}
