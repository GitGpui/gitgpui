use std::ops::Range;

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

fn tokenize_for_word_diff(s: &str) -> Vec<Token> {
    fn classify(c: char) -> (u8, TokenKind) {
        if c.is_whitespace() {
            return (0, TokenKind::Whitespace);
        }
        if c.is_alphanumeric() || c == '_' {
            return (1, TokenKind::Other);
        }
        (2, TokenKind::Other)
    }

    let mut out = Vec::new();
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
    let old_tokens = tokenize_for_word_diff(old);
    let new_tokens = tokenize_for_word_diff(new);

    const MAX_TOKENS: usize = 128;
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
    let mut trace: Vec<Vec<isize>> = Vec::new();

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
