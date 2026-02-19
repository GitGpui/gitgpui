use std::ops::Range;

const WORD_DIFF_MAX_BYTES_PER_SIDE: usize = 4 * 1024;
const WORD_DIFF_MAX_TOTAL_BYTES: usize = 8 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TokenKind {
    Whitespace,
    Other,
}

#[derive(Clone, Debug)]
struct Token {
    range: Range<usize>,
    kind: TokenKind,
}

fn tokenize_for_word_diff(s: &str, max_tokens: usize) -> Vec<Token> {
    if max_tokens == 0 {
        return Vec::new();
    }

    fn classify(c: char) -> (u8, TokenKind) {
        if c.is_whitespace() {
            return (0, TokenKind::Whitespace);
        }
        if c.is_alphanumeric() || c == '_' {
            return (1, TokenKind::Other);
        }
        (2, TokenKind::Other)
    }

    let mut out = Vec::with_capacity(max_tokens);
    let mut it = s.char_indices().peekable();
    while let Some((start, ch)) = it.next() {
        let (class, kind) = classify(ch);
        let mut end = start + ch.len_utf8();
        while let Some(&(next_start, next_ch)) = it.peek() {
            let (next_class, _) = classify(next_ch);
            if next_class != class {
                break;
            }
            it.next();
            end = next_start + next_ch.len_utf8();
        }
        out.push(Token {
            range: start..end,
            kind,
        });
        if out.len() >= max_tokens {
            break;
        }
    }
    out
}

fn coalesce_ranges(mut ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    if ranges.len() <= 1 {
        return ranges;
    }
    ranges.sort_by_key(|r| (r.start, r.end));
    let mut out: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
    for r in ranges {
        if let Some(last) = out.last_mut()
            && r.start <= last.end
        {
            last.end = last.end.max(r.end);
            continue;
        }
        out.push(r);
    }
    out
}

pub(super) fn word_diff_ranges(old: &str, new: &str) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
    const MAX_TOKENS: usize = 128;
    let old_tokens = tokenize_for_word_diff(old, MAX_TOKENS + 1);
    let new_tokens = tokenize_for_word_diff(new, MAX_TOKENS + 1);
    if old_tokens.len() > MAX_TOKENS || new_tokens.len() > MAX_TOKENS {
        return fallback_affix_diff_ranges(old, new);
    }
    if old_tokens.is_empty() || new_tokens.is_empty() {
        return fallback_affix_diff_ranges(old, new);
    }

    let old_slices: Vec<&str> = old_tokens
        .iter()
        .map(|t| &old[t.range.clone()])
        .collect::<Vec<_>>();
    let new_slices: Vec<&str> = new_tokens
        .iter()
        .map(|t| &new[t.range.clone()])
        .collect::<Vec<_>>();

    // Compute the longest common subsequence via Myers' diff algorithm, marking matching tokens
    // as "kept". This is substantially faster than O(n*m) DP for typical lines.
    let n = old_slices.len() as isize;
    let m = new_slices.len() as isize;
    let max = (n + m) as usize;
    let offset = max as isize;

    let mut v: Vec<isize> = vec![0; 2 * max + 1];
    let mut trace: Vec<Vec<isize>> = Vec::with_capacity(max + 1);

    let mut done = false;
    for d in 0..=max {
        for k in (-(d as isize)..=(d as isize)).step_by(2) {
            let k_ix = (k + offset) as usize;
            let x = if k == -(d as isize)
                || (k != d as isize && v[(k - 1 + offset) as usize] < v[(k + 1 + offset) as usize])
            {
                v[(k + 1 + offset) as usize]
            } else {
                v[(k - 1 + offset) as usize] + 1
            };

            let mut x = x;
            let mut y = x - k;
            while x < n && y < m && old_slices[x as usize] == new_slices[y as usize] {
                x += 1;
                y += 1;
            }

            v[k_ix] = x;
            if x >= n && y >= m {
                done = true;
                break;
            }
        }

        trace.push(v.clone());
        if done {
            break;
        }
    }

    let mut keep_old = vec![false; old_tokens.len()];
    let mut keep_new = vec![false; new_tokens.len()];

    let mut x = n;
    let mut y = m;

    for d in (1..trace.len()).rev() {
        let d_isize = d as isize;
        let v = &trace[d - 1];
        let k = x - y;
        let prev_k = if k == -d_isize
            || (k != d_isize && v[(k - 1 + offset) as usize] < v[(k + 1 + offset) as usize])
        {
            k + 1
        } else {
            k - 1
        };

        let prev_x = v[(prev_k + offset) as usize];
        let prev_y = prev_x - prev_k;

        while x > prev_x && y > prev_y {
            keep_old[(x - 1) as usize] = true;
            keep_new[(y - 1) as usize] = true;
            x -= 1;
            y -= 1;
        }

        // Step to the previous edit.
        if x == prev_x {
            y -= 1;
        } else {
            x -= 1;
        }
    }

    while x > 0 && y > 0 {
        if old_slices[(x - 1) as usize] != new_slices[(y - 1) as usize] {
            break;
        }
        keep_old[(x - 1) as usize] = true;
        keep_new[(y - 1) as usize] = true;
        x -= 1;
        y -= 1;
    }

    let old_ranges = old_tokens
        .iter()
        .zip(keep_old.iter().copied())
        .filter_map(|(t, keep)| (!keep && t.kind == TokenKind::Other).then_some(t.range.clone()))
        .collect::<Vec<_>>();
    let new_ranges = new_tokens
        .iter()
        .zip(keep_new.iter().copied())
        .filter_map(|(t, keep)| (!keep && t.kind == TokenKind::Other).then_some(t.range.clone()))
        .collect::<Vec<_>>();

    (coalesce_ranges(old_ranges), coalesce_ranges(new_ranges))
}

pub(super) fn capped_word_diff_ranges(
    old: &str,
    new: &str,
) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
    if old.len() > WORD_DIFF_MAX_BYTES_PER_SIDE
        || new.len() > WORD_DIFF_MAX_BYTES_PER_SIDE
        || old.len().saturating_add(new.len()) > WORD_DIFF_MAX_TOTAL_BYTES
    {
        return (Vec::new(), Vec::new());
    }
    word_diff_ranges(old, new)
}

fn fallback_affix_diff_ranges(old: &str, new: &str) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
    let mut prefix = 0usize;
    for ((old_ix, old_ch), (_new_ix, new_ch)) in old.char_indices().zip(new.char_indices()) {
        if old_ch != new_ch {
            break;
        }
        prefix = old_ix + old_ch.len_utf8();
    }

    let mut suffix = 0usize;
    let old_tail = &old[prefix.min(old.len())..];
    let new_tail = &new[prefix.min(new.len())..];
    for (old_ch, new_ch) in old_tail.chars().rev().zip(new_tail.chars().rev()) {
        if old_ch != new_ch {
            break;
        }
        suffix += old_ch.len_utf8();
    }

    let old_mid_start = prefix.min(old.len());
    let old_mid_end = old.len().saturating_sub(suffix).max(old_mid_start);
    let new_mid_start = prefix.min(new.len());
    let new_mid_end = new.len().saturating_sub(suffix).max(new_mid_start);

    let old_ranges = if old_mid_end > old_mid_start {
        vec![old_mid_start..old_mid_end]
    } else {
        Vec::new()
    };
    let new_ranges = if new_mid_end > new_mid_start {
        vec![new_mid_start..new_mid_end]
    } else {
        Vec::new()
    };
    (old_ranges, new_ranges)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn word_diff_ranges_highlights_changed_tokens() {
        let (old, new) = ("let x = 1;", "let x = 2;");
        let (old_ranges, new_ranges) = word_diff_ranges(old, new);
        assert_eq!(
            old_ranges
                .iter()
                .map(|r| &old[r.clone()])
                .collect::<Vec<_>>(),
            vec!["1"]
        );
        assert_eq!(
            new_ranges
                .iter()
                .map(|r| &new[r.clone()])
                .collect::<Vec<_>>(),
            vec!["2"]
        );
    }

    #[test]
    fn capped_word_diff_ranges_matches_word_diff_for_small_inputs() {
        let (old, new) = ("let x = 1;", "let x = 2;");
        let (a_old, a_new) = word_diff_ranges(old, new);
        let (b_old, b_new) = capped_word_diff_ranges(old, new);
        assert_eq!(a_old, b_old);
        assert_eq!(a_new, b_new);
    }

    #[test]
    fn capped_word_diff_ranges_skips_huge_inputs() {
        let old = "a".repeat(WORD_DIFF_MAX_TOTAL_BYTES + 1);
        let new = format!("{old}x");
        let (old_ranges, new_ranges) = capped_word_diff_ranges(&old, &new);
        assert!(old_ranges.is_empty());
        assert!(new_ranges.is_empty());
    }

    #[test]
    fn word_diff_ranges_handles_unicode_safely() {
        let (old, new) = ("aé", "aê");
        let (old_ranges, new_ranges) = word_diff_ranges(old, new);
        assert_eq!(
            old_ranges
                .iter()
                .map(|r| &old[r.clone()])
                .collect::<Vec<_>>(),
            vec!["aé"]
        );
        assert_eq!(
            new_ranges
                .iter()
                .map(|r| &new[r.clone()])
                .collect::<Vec<_>>(),
            vec!["aê"]
        );
    }

    #[test]
    fn word_diff_ranges_falls_back_for_large_inputs() {
        let old = "a".repeat(2048);
        let new = format!("{old}x");
        let (old_ranges, new_ranges) = word_diff_ranges(&old, &new);
        assert!(old_ranges.len() <= 1);
        assert!(new_ranges.len() <= 1);
    }

    #[test]
    fn word_diff_ranges_outputs_are_ordered_and_utf8_safe() {
        let (old, new) = ("aé b", "aê  b");
        let (old_ranges, new_ranges) = word_diff_ranges(old, new);

        for r in &old_ranges {
            assert!(r.start <= r.end);
            assert!(r.end <= old.len());
            assert!(old.is_char_boundary(r.start));
            assert!(old.is_char_boundary(r.end));
        }
        for w in old_ranges.windows(2) {
            assert!(w[0].end <= w[1].start);
        }

        for r in &new_ranges {
            assert!(r.start <= r.end);
            assert!(r.end <= new.len());
            assert!(new.is_char_boundary(r.start));
            assert!(new.is_char_boundary(r.end));
        }
        for w in new_ranges.windows(2) {
            assert!(w[0].end <= w[1].start);
        }
    }

    #[test]
    fn word_diff_ranges_empty_inputs_do_not_panic() {
        let (old_ranges, new_ranges) = word_diff_ranges("", "");
        assert!(old_ranges.is_empty());
        assert!(new_ranges.is_empty());
    }

    #[test]
    #[ignore]
    fn perf_word_diff_ranges_smoke() {
        let old = "fn foo(a: i32, b: i32) -> i32 { a + b }";
        let new = "fn foo(a: i32, b: i32) -> i32 { a - b }";
        let start = Instant::now();
        for _ in 0..200_000 {
            let _ = word_diff_ranges(old, new);
        }
        eprintln!("word_diff_ranges: {:?}", start.elapsed());
    }
}
