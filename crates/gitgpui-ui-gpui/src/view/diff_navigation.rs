pub(super) fn diff_nav_prev_target(entries: &[usize], current: usize) -> Option<usize> {
    entries.iter().rev().find(|&&ix| ix < current).copied()
}

pub(super) fn diff_nav_next_target(entries: &[usize], current: usize) -> Option<usize> {
    entries.iter().find(|&&ix| ix > current).copied()
}

pub(super) fn change_block_entries(
    len: usize,
    mut is_change: impl FnMut(usize) -> bool,
) -> Vec<usize> {
    let mut out = Vec::new();
    let mut in_block = false;
    for ix in 0..len {
        let is_change = is_change(ix);
        if is_change && !in_block {
            out.push(ix);
            in_block = true;
        } else if !is_change {
            in_block = false;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_nav_prev_next_targets_do_not_wrap() {
        let entries = vec![10, 20, 30];

        assert_eq!(diff_nav_prev_target(&entries, 10), None);
        assert_eq!(diff_nav_next_target(&entries, 30), None);

        assert_eq!(diff_nav_prev_target(&entries, 25), Some(20));
        assert_eq!(diff_nav_next_target(&entries, 25), Some(30));

        assert_eq!(diff_nav_next_target(&entries, 0), Some(10));
        assert_eq!(diff_nav_prev_target(&entries, 100), Some(30));
    }
}
