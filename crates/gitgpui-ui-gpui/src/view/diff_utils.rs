use super::*;

pub(super) fn scrollbar_markers_from_flags(
    len: usize,
    mut flag_at_index: impl FnMut(usize) -> u8,
) -> Vec<zed::ScrollbarMarker> {
    if len == 0 {
        return Vec::new();
    }

    let bucket_count = 240usize.min(len).max(1);
    let mut buckets = vec![0u8; bucket_count];
    for ix in 0..len {
        let flag = flag_at_index(ix);
        if flag == 0 {
            continue;
        }
        let b = (ix * bucket_count) / len;
        if let Some(cell) = buckets.get_mut(b) {
            *cell |= flag;
        }
    }

    let mut out = Vec::new();
    let mut ix = 0usize;
    while ix < bucket_count {
        let flag = buckets[ix];
        if flag == 0 {
            ix += 1;
            continue;
        }

        let start = ix;
        ix += 1;
        while ix < bucket_count && buckets[ix] == flag {
            ix += 1;
        }
        let end = ix; // exclusive

        let kind = match flag {
            1 => zed::ScrollbarMarkerKind::Add,
            2 => zed::ScrollbarMarkerKind::Remove,
            _ => zed::ScrollbarMarkerKind::Modify,
        };

        out.push(zed::ScrollbarMarker {
            start: start as f32 / bucket_count as f32,
            end: end as f32 / bucket_count as f32,
            kind,
        });
    }

    out
}

pub(super) fn diff_content_text(line: &AnnotatedDiffLine) -> &str {
    match line.kind {
        gitgpui_core::domain::DiffLineKind::Add => {
            line.text.strip_prefix('+').unwrap_or(&line.text)
        }
        gitgpui_core::domain::DiffLineKind::Remove => {
            line.text.strip_prefix('-').unwrap_or(&line.text)
        }
        gitgpui_core::domain::DiffLineKind::Context => {
            line.text.strip_prefix(' ').unwrap_or(&line.text)
        }
        gitgpui_core::domain::DiffLineKind::Header | gitgpui_core::domain::DiffLineKind::Hunk => {
            &line.text
        }
    }
}

pub(super) fn parse_diff_git_header_path(text: &str) -> Option<String> {
    let text = text.strip_prefix("diff --git ")?;
    let mut parts = text.split_whitespace();
    let a = parts.next()?;
    let b = parts.next().unwrap_or(a);
    let b = b.strip_prefix("b/").unwrap_or(b);
    Some(b.to_string())
}

#[derive(Clone, Debug)]
pub(super) struct ParsedHunkHeader {
    pub(super) old: String,
    pub(super) new: String,
    pub(super) heading: Option<String>,
}

pub(super) fn parse_unified_hunk_header_for_display(text: &str) -> Option<ParsedHunkHeader> {
    let text = text.strip_prefix("@@")?.trim_start();
    let (ranges, rest) = text.split_once("@@")?;
    let ranges = ranges.trim();
    let heading = rest.trim();

    let mut it = ranges.split_whitespace();
    let old = it.next()?.trim().to_string();
    let new = it.next()?.trim().to_string();

    Some(ParsedHunkHeader {
        old,
        new,
        heading: (!heading.is_empty()).then_some(heading.to_string()),
    })
}

pub(super) fn compute_diff_file_stats(diff: &[AnnotatedDiffLine]) -> Vec<Option<(usize, usize)>> {
    let mut stats: Vec<Option<(usize, usize)>> = vec![None; diff.len()];

    let mut current_file_header_ix: Option<usize> = None;
    let mut adds = 0usize;
    let mut removes = 0usize;

    for (ix, line) in diff.iter().enumerate() {
        let is_file_header = matches!(line.kind, gitgpui_core::domain::DiffLineKind::Header)
            && line.text.starts_with("diff --git ");

        if is_file_header {
            if let Some(header_ix) = current_file_header_ix.take() {
                stats[header_ix] = Some((adds, removes));
            }
            current_file_header_ix = Some(ix);
            adds = 0;
            removes = 0;
            continue;
        }

        match line.kind {
            gitgpui_core::domain::DiffLineKind::Add => adds += 1,
            gitgpui_core::domain::DiffLineKind::Remove => removes += 1,
            _ => {}
        }
    }

    if let Some(header_ix) = current_file_header_ix {
        stats[header_ix] = Some((adds, removes));
    }

    stats
}

pub(super) fn compute_diff_file_for_src_ix(diff: &[AnnotatedDiffLine]) -> Vec<Option<Arc<str>>> {
    let mut out: Vec<Option<Arc<str>>> = Vec::with_capacity(diff.len());
    let mut current_file: Option<Arc<str>> = None;

    for line in diff {
        let is_file_header = matches!(line.kind, gitgpui_core::domain::DiffLineKind::Header)
            && line.text.starts_with("diff --git ");
        if is_file_header {
            current_file = parse_diff_git_header_path(&line.text).map(Arc::<str>::from);
        }
        out.push(current_file.clone());
    }

    out
}

pub(super) fn enclosing_hunk_src_ix(diff: &[AnnotatedDiffLine], src_ix: usize) -> Option<usize> {
    let src_ix = src_ix.min(diff.len().saturating_sub(1));
    for ix in (0..=src_ix).rev() {
        let line = diff.get(ix)?;
        if matches!(line.kind, gitgpui_core::domain::DiffLineKind::Header)
            && line.text.starts_with("diff --git ")
        {
            break;
        }
        if matches!(line.kind, gitgpui_core::domain::DiffLineKind::Hunk) {
            return Some(ix);
        }
    }
    None
}

pub(super) fn build_unified_patch_for_hunk(
    diff: &[AnnotatedDiffLine],
    hunk_src_ix: usize,
) -> Option<String> {
    let lines = diff;
    let hunk = lines.get(hunk_src_ix)?;
    if !matches!(hunk.kind, gitgpui_core::domain::DiffLineKind::Hunk) {
        return None;
    }

    let file_start = (0..=hunk_src_ix).rev().find(|&ix| {
        lines
            .get(ix)
            .is_some_and(|l| l.text.starts_with("diff --git "))
    })?;

    let first_hunk = (file_start + 1..lines.len())
        .find(|&ix| {
            let Some(line) = lines.get(ix) else {
                return false;
            };
            matches!(line.kind, gitgpui_core::domain::DiffLineKind::Hunk)
                || line.text.starts_with("diff --git ")
        })
        .unwrap_or(lines.len());

    let header_end = first_hunk.min(hunk_src_ix);
    let hunk_end = (hunk_src_ix + 1..lines.len())
        .find(|&ix| {
            let Some(line) = lines.get(ix) else {
                return false;
            };
            matches!(line.kind, gitgpui_core::domain::DiffLineKind::Hunk)
                || line.text.starts_with("diff --git ")
        })
        .unwrap_or(lines.len());

    let mut out = String::new();
    for line in &lines[file_start..header_end] {
        out.push_str(&line.text);
        out.push('\n');
    }
    for line in &lines[hunk_src_ix..hunk_end] {
        out.push_str(&line.text);
        out.push('\n');
    }
    (!out.trim().is_empty()).then_some(out)
}

pub(super) fn build_unified_patch_for_hunks(
    diff: &[AnnotatedDiffLine],
    hunk_src_ixs: &[usize],
) -> Option<String> {
    if hunk_src_ixs.is_empty() {
        return None;
    }

    let mut hunks = hunk_src_ixs.to_vec();
    hunks.sort_unstable();
    hunks.dedup();

    let mut out = String::new();
    for hunk_src_ix in hunks {
        let Some(patch) = build_unified_patch_for_hunk(diff, hunk_src_ix) else {
            continue;
        };
        out.push_str(&patch);
    }

    (!out.trim().is_empty()).then_some(out)
}

pub(super) fn build_unified_patch_for_hunk_selection(
    diff: &[AnnotatedDiffLine],
    hunk_src_ix: usize,
    selected_src_ixs: &std::collections::HashSet<usize>,
) -> Option<String> {
    if selected_src_ixs.is_empty() {
        return None;
    }

    let lines = diff;
    let hunk = lines.get(hunk_src_ix)?;
    if !matches!(hunk.kind, gitgpui_core::domain::DiffLineKind::Hunk) {
        return None;
    }

    let file_start = (0..=hunk_src_ix).rev().find(|&ix| {
        lines
            .get(ix)
            .is_some_and(|l| l.text.starts_with("diff --git "))
    })?;

    let first_hunk = (file_start + 1..lines.len())
        .find(|&ix| {
            let Some(line) = lines.get(ix) else {
                return false;
            };
            matches!(line.kind, gitgpui_core::domain::DiffLineKind::Hunk)
                || line.text.starts_with("diff --git ")
        })
        .unwrap_or(lines.len());

    let header_end = first_hunk.min(hunk_src_ix);
    let hunk_end = (hunk_src_ix + 1..lines.len())
        .find(|&ix| {
            let Some(line) = lines.get(ix) else {
                return false;
            };
            matches!(line.kind, gitgpui_core::domain::DiffLineKind::Hunk)
                || line.text.starts_with("diff --git ")
        })
        .unwrap_or(lines.len());

    let mut out = String::new();
    for line in &lines[file_start..header_end] {
        out.push_str(&line.text);
        out.push('\n');
    }

    // Keep the original hunk header; `git apply --recount` will adjust counts.
    out.push_str(&lines[hunk_src_ix].text);
    out.push('\n');

    let mut has_change = false;
    let mut prev_included = false;
    for ix in hunk_src_ix + 1..hunk_end {
        let line = &lines[ix];

        if line.text.starts_with("\\") {
            if prev_included {
                out.push_str(&line.text);
                out.push('\n');
            }
            continue;
        }

        match line.kind {
            gitgpui_core::domain::DiffLineKind::Add => {
                if selected_src_ixs.contains(&ix) {
                    out.push_str(&line.text);
                    out.push('\n');
                    has_change = true;
                    prev_included = true;
                } else {
                    prev_included = false;
                }
            }
            gitgpui_core::domain::DiffLineKind::Remove => {
                if selected_src_ixs.contains(&ix) {
                    out.push_str(&line.text);
                    out.push('\n');
                    has_change = true;
                    prev_included = true;
                } else {
                    let content = line.text.strip_prefix('-').unwrap_or(&line.text);
                    out.push(' ');
                    out.push_str(content);
                    out.push('\n');
                    prev_included = true;
                }
            }
            gitgpui_core::domain::DiffLineKind::Context => {
                out.push_str(&line.text);
                out.push('\n');
                prev_included = true;
            }
            gitgpui_core::domain::DiffLineKind::Header
            | gitgpui_core::domain::DiffLineKind::Hunk => {
                out.push_str(&line.text);
                out.push('\n');
                prev_included = true;
            }
        }
    }

    has_change.then_some(out)
}

pub(super) fn build_unified_patch_for_hunk_selection_for_worktree_discard(
    diff: &[AnnotatedDiffLine],
    hunk_src_ix: usize,
    selected_src_ixs: &std::collections::HashSet<usize>,
) -> Option<String> {
    if selected_src_ixs.is_empty() {
        return None;
    }

    let lines = diff;
    let hunk = lines.get(hunk_src_ix)?;
    if !matches!(hunk.kind, gitgpui_core::domain::DiffLineKind::Hunk) {
        return None;
    }

    let file_start = (0..=hunk_src_ix).rev().find(|&ix| {
        lines
            .get(ix)
            .is_some_and(|l| l.text.starts_with("diff --git "))
    })?;

    let first_hunk = (file_start + 1..lines.len())
        .find(|&ix| {
            let Some(line) = lines.get(ix) else {
                return false;
            };
            matches!(line.kind, gitgpui_core::domain::DiffLineKind::Hunk)
                || line.text.starts_with("diff --git ")
        })
        .unwrap_or(lines.len());

    let header_end = first_hunk.min(hunk_src_ix);
    let hunk_end = (hunk_src_ix + 1..lines.len())
        .find(|&ix| {
            let Some(line) = lines.get(ix) else {
                return false;
            };
            matches!(line.kind, gitgpui_core::domain::DiffLineKind::Hunk)
                || line.text.starts_with("diff --git ")
        })
        .unwrap_or(lines.len());

    let mut out = String::new();
    for line in &lines[file_start..header_end] {
        out.push_str(&line.text);
        out.push('\n');
    }

    // Keep the original hunk header; `git apply --recount` will adjust counts.
    out.push_str(&lines[hunk_src_ix].text);
    out.push('\n');

    let mut has_change = false;
    let mut prev_included = false;
    for ix in hunk_src_ix + 1..hunk_end {
        let line = &lines[ix];

        if line.text.starts_with("\\") {
            if prev_included {
                out.push_str(&line.text);
                out.push('\n');
            }
            continue;
        }

        match line.kind {
            gitgpui_core::domain::DiffLineKind::Add => {
                if selected_src_ixs.contains(&ix) {
                    out.push_str(&line.text);
                    out.push('\n');
                    has_change = true;
                    prev_included = true;
                } else {
                    let content = line.text.strip_prefix('+').unwrap_or(&line.text);
                    out.push(' ');
                    out.push_str(content);
                    out.push('\n');
                    prev_included = true;
                }
            }
            gitgpui_core::domain::DiffLineKind::Remove => {
                if selected_src_ixs.contains(&ix) {
                    out.push_str(&line.text);
                    out.push('\n');
                    has_change = true;
                    prev_included = true;
                } else {
                    prev_included = false;
                }
            }
            gitgpui_core::domain::DiffLineKind::Context => {
                out.push_str(&line.text);
                out.push('\n');
                prev_included = true;
            }
            gitgpui_core::domain::DiffLineKind::Header
            | gitgpui_core::domain::DiffLineKind::Hunk => {
                out.push_str(&line.text);
                out.push('\n');
                prev_included = true;
            }
        }
    }

    has_change.then_some(out)
}

pub(super) fn build_unified_patch_for_selected_lines_across_hunks(
    diff: &[AnnotatedDiffLine],
    selected_src_ixs: &std::collections::HashSet<usize>,
) -> Option<String> {
    use gitgpui_core::domain::DiffLineKind as K;
    use std::collections::{BTreeMap, HashSet};

    if selected_src_ixs.is_empty() {
        return None;
    }

    let mut by_hunk: BTreeMap<usize, HashSet<usize>> = BTreeMap::new();
    for &src_ix in selected_src_ixs {
        let Some(line) = diff.get(src_ix) else {
            continue;
        };
        if !matches!(line.kind, K::Add | K::Remove) {
            continue;
        }
        let Some(hunk_src_ix) = enclosing_hunk_src_ix(diff, src_ix) else {
            continue;
        };
        by_hunk.entry(hunk_src_ix).or_default().insert(src_ix);
    }

    let mut out = String::new();
    for (hunk_src_ix, src_ixs) in by_hunk {
        let Some(patch) = build_unified_patch_for_hunk_selection(diff, hunk_src_ix, &src_ixs)
        else {
            continue;
        };
        out.push_str(&patch);
    }

    (!out.trim().is_empty()).then_some(out)
}

pub(super) fn build_unified_patch_for_selected_lines_across_hunks_for_worktree_discard(
    diff: &[AnnotatedDiffLine],
    selected_src_ixs: &std::collections::HashSet<usize>,
) -> Option<String> {
    use gitgpui_core::domain::DiffLineKind as K;
    use std::collections::{BTreeMap, HashSet};

    if selected_src_ixs.is_empty() {
        return None;
    }

    let mut by_hunk: BTreeMap<usize, HashSet<usize>> = BTreeMap::new();
    for &src_ix in selected_src_ixs {
        let Some(line) = diff.get(src_ix) else {
            continue;
        };
        if !matches!(line.kind, K::Add | K::Remove) {
            continue;
        }
        let Some(hunk_src_ix) = enclosing_hunk_src_ix(diff, src_ix) else {
            continue;
        };
        by_hunk.entry(hunk_src_ix).or_default().insert(src_ix);
    }

    let mut out = String::new();
    for (hunk_src_ix, src_ixs) in by_hunk {
        let Some(patch) = build_unified_patch_for_hunk_selection_for_worktree_discard(
            diff,
            hunk_src_ix,
            &src_ixs,
        ) else {
            continue;
        };
        out.push_str(&patch);
    }

    (!out.trim().is_empty()).then_some(out)
}

pub(super) fn context_menu_selection_range_from_diff_text(
    selection: Option<(DiffTextPos, DiffTextPos)>,
    diff_view: DiffViewMode,
    clicked_visible_ix: usize,
    clicked_region: DiffTextRegion,
) -> Option<(usize, usize)> {
    let (start, end) = selection?;
    if start == end {
        return None;
    }
    if clicked_visible_ix < start.visible_ix || clicked_visible_ix > end.visible_ix {
        return None;
    }

    let restrict_region = (diff_view == DiffViewMode::Split
        && start.region == end.region
        && matches!(
            start.region,
            DiffTextRegion::SplitLeft | DiffTextRegion::SplitRight
        ))
    .then_some(start.region);
    if restrict_region.is_some_and(|r| r != clicked_region) {
        return None;
    }

    Some((start.visible_ix, end.visible_ix))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gitgpui_core::domain::DiffLineKind as K;
    use std::collections::HashSet;

    fn dl(kind: K, text: &str) -> AnnotatedDiffLine {
        AnnotatedDiffLine {
            kind,
            text: text.into(),
            old_line: None,
            new_line: None,
        }
    }

    fn example_two_hunk_diff() -> Vec<AnnotatedDiffLine> {
        vec![
            dl(K::Header, "diff --git a/file.txt b/file.txt"),
            dl(K::Header, "index 1111111..2222222 100644"),
            dl(K::Header, "--- a/file.txt"),
            dl(K::Header, "+++ b/file.txt"),
            dl(K::Hunk, "@@ -1,3 +1,3 @@"),
            dl(K::Context, " line1"),
            dl(K::Remove, "-line2"),
            dl(K::Add, "+line2_mod"),
            dl(K::Context, " line3"),
            dl(K::Hunk, "@@ -5,3 +5,4 @@"),
            dl(K::Context, " line5"),
            dl(K::Context, " line6"),
            dl(K::Add, "+line6_5"),
            dl(K::Context, " line7"),
        ]
    }

    fn example_two_file_diff() -> Vec<AnnotatedDiffLine> {
        vec![
            dl(K::Header, "diff --git a/a.txt b/a.txt"),
            dl(K::Header, "--- a/a.txt"),
            dl(K::Header, "+++ b/a.txt"),
            dl(K::Hunk, "@@ -1,0 +1,1 @@"),
            dl(K::Add, "+a"),
            dl(K::Header, "diff --git a/b.txt b/b.txt"),
            dl(K::Header, "--- a/b.txt"),
            dl(K::Header, "+++ b/b.txt"),
            dl(K::Hunk, "@@ -1,0 +1,1 @@"),
            dl(K::Add, "+b"),
        ]
    }

    fn example_two_mods_one_hunk_diff() -> Vec<AnnotatedDiffLine> {
        vec![
            dl(K::Header, "diff --git a/file.txt b/file.txt"),
            dl(K::Header, "index 1111111..2222222 100644"),
            dl(K::Header, "--- a/file.txt"),
            dl(K::Header, "+++ b/file.txt"),
            dl(K::Hunk, "@@ -1,4 +1,4 @@"),
            dl(K::Context, " line1"),
            dl(K::Remove, "-line2"),
            dl(K::Add, "+line2_mod"),
            dl(K::Remove, "-line3"),
            dl(K::Add, "+line3_mod"),
            dl(K::Context, " line4"),
        ]
    }

    #[test]
    fn build_unified_patch_for_hunks_includes_multiple_hunks() {
        let diff = example_two_hunk_diff();
        let patch = build_unified_patch_for_hunks(&diff, &[4, 9]).expect("patch");
        assert!(patch.contains("@@ -1,3 +1,3 @@"));
        assert!(patch.contains("@@ -5,3 +5,4 @@"));
    }

    #[test]
    fn build_unified_patch_for_selected_lines_across_hunks_includes_all_selected_lines() {
        let diff = example_two_hunk_diff();
        let selected: HashSet<usize> = [6, 7, 12].into_iter().collect();

        let patch =
            build_unified_patch_for_selected_lines_across_hunks(&diff, &selected).expect("patch");
        assert!(patch.contains("@@ -1,3 +1,3 @@"));
        assert!(patch.contains("@@ -5,3 +5,4 @@"));
        assert!(patch.contains("-line2"));
        assert!(patch.contains("+line2_mod"));
        assert!(patch.contains("+line6_5"));
    }

    #[test]
    fn build_unified_patch_for_selected_lines_across_hunks_ignores_context_only_selection() {
        let diff = example_two_hunk_diff();
        let selected: HashSet<usize> = [5, 8, 10].into_iter().collect();

        assert!(build_unified_patch_for_selected_lines_across_hunks(&diff, &selected).is_none());
    }

    #[test]
    fn build_unified_patch_for_selected_lines_across_hunks_supports_multiple_files() {
        let diff = example_two_file_diff();
        let selected: HashSet<usize> = [4, 9].into_iter().collect();

        let patch =
            build_unified_patch_for_selected_lines_across_hunks(&diff, &selected).expect("patch");
        assert!(patch.contains("diff --git a/a.txt b/a.txt"));
        assert!(patch.contains("diff --git a/b.txt b/b.txt"));
        assert!(patch.contains("+a"));
        assert!(patch.contains("+b"));
    }

    #[test]
    fn build_unified_patch_for_selected_lines_across_hunks_for_worktree_discard_keeps_unselected_changes_as_worktree_context()
     {
        let diff = example_two_mods_one_hunk_diff();
        let selected: HashSet<usize> = [6, 7].into_iter().collect();

        let patch = build_unified_patch_for_selected_lines_across_hunks_for_worktree_discard(
            &diff, &selected,
        )
        .expect("patch");

        assert!(patch.contains("-line2"));
        assert!(patch.contains("+line2_mod"));
        assert!(!patch.contains("-line3"));
        assert!(!patch.contains("+line3_mod"));
        assert!(patch.contains(" line3_mod"));
    }

    #[test]
    fn context_menu_selection_range_from_diff_text_requires_click_in_selection() {
        let selection = Some((
            DiffTextPos {
                visible_ix: 2,
                region: DiffTextRegion::Inline,
                offset: 0,
            },
            DiffTextPos {
                visible_ix: 5,
                region: DiffTextRegion::Inline,
                offset: 3,
            },
        ));
        assert_eq!(
            context_menu_selection_range_from_diff_text(
                selection,
                DiffViewMode::Inline,
                4,
                DiffTextRegion::Inline
            ),
            Some((2, 5))
        );
        assert_eq!(
            context_menu_selection_range_from_diff_text(
                selection,
                DiffViewMode::Inline,
                1,
                DiffTextRegion::Inline
            ),
            None
        );
    }

    #[test]
    fn context_menu_selection_range_from_diff_text_restricts_split_region() {
        let selection = Some((
            DiffTextPos {
                visible_ix: 1,
                region: DiffTextRegion::SplitLeft,
                offset: 0,
            },
            DiffTextPos {
                visible_ix: 3,
                region: DiffTextRegion::SplitLeft,
                offset: 2,
            },
        ));
        assert_eq!(
            context_menu_selection_range_from_diff_text(
                selection,
                DiffViewMode::Split,
                2,
                DiffTextRegion::SplitLeft
            ),
            Some((1, 3))
        );
        assert_eq!(
            context_menu_selection_range_from_diff_text(
                selection,
                DiffViewMode::Split,
                2,
                DiffTextRegion::SplitRight
            ),
            None
        );
    }
}
