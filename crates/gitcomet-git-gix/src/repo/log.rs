use super::history::gix_head_id_or_none;
use super::{GixRepo, bstr_to_arc_str, oid_to_arc_str};
use crate::util::{
    bytes_to_text_preserving_utf8, parse_git_log_pretty_records, path_buf_from_git_bytes,
    run_git_capture, unix_seconds_to_system_time, unix_seconds_to_system_time_or_epoch,
};
use gitcomet_core::domain::{
    Commit, CommitDetails, CommitFileChange, CommitId, CommitParentIds, LogCursor, LogPage,
    ReflogEntry, StashEntry,
};
use gitcomet_core::error::{Error, ErrorKind, GitFailure, GitFailureId};
use gitcomet_core::history_query::{HistoryQuery, HistoryQueryField, HistoryQueryTerm};
use gitcomet_core::services::Result;
use gix::bstr::ByteSlice as _;
use gix::objs::FindExt as _;
use gix::traverse::commit::simple::CommitTimeOrder;
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::path::Path;
use std::sync::Arc;

struct CursorGate<'a> {
    last_seen: Option<&'a str>,
    started: bool,
}

impl<'a> CursorGate<'a> {
    fn new(cursor: Option<&'a LogCursor>) -> Self {
        Self {
            last_seen: cursor.map(|cursor| cursor.last_seen.as_ref()),
            started: cursor.is_none(),
        }
    }

    fn should_skip(&mut self, id: &str) -> bool {
        self.should_skip_hex(id)
    }

    fn should_skip_oid(&mut self, id: &gix::oid) -> bool {
        if self.started {
            return false;
        }

        let mut buf = gix::hash::Kind::hex_buf();
        self.should_skip_hex(id.hex_to_buf(&mut buf))
    }

    fn should_skip_hex(&mut self, id: &str) -> bool {
        if self.started {
            return false;
        }

        let Some(last_seen) = self.last_seen else {
            self.started = true;
            return false;
        };

        if last_seen == id {
            self.started = true;
        }

        true
    }
}

#[derive(Clone, Copy)]
struct AsciiCaseInsensitiveNeedle<'a> {
    bytes: &'a [u8],
    first_lower: u8,
    first_upper: u8,
    last_lower: u8,
    last_upper: u8,
}

impl<'a> AsciiCaseInsensitiveNeedle<'a> {
    fn new(needle: &'a str) -> Option<Self> {
        let bytes = needle.as_bytes();
        let (&first, &last) = bytes.first().zip(bytes.last())?;
        Some(Self {
            bytes,
            first_lower: first.to_ascii_lowercase(),
            first_upper: first.to_ascii_uppercase(),
            last_lower: last.to_ascii_lowercase(),
            last_upper: last.to_ascii_uppercase(),
        })
    }

    fn is_match(self, haystack: &str) -> bool {
        let haystack_bytes = haystack.as_bytes();
        let needle_len = self.bytes.len();
        let Some(last_start) = haystack_bytes.len().checked_sub(needle_len) else {
            return false;
        };

        'outer: for start in 0..=last_start {
            let first = haystack_bytes[start];
            if first != self.first_lower && first != self.first_upper {
                continue;
            }

            if needle_len == 1 {
                return true;
            }

            let last = haystack_bytes[start + needle_len - 1];
            if last != self.last_lower && last != self.last_upper {
                continue;
            }

            for (offset, needle_byte) in self.bytes[1..needle_len - 1].iter().copied().enumerate() {
                if !haystack_bytes[start + offset + 1].eq_ignore_ascii_case(&needle_byte) {
                    continue 'outer;
                }
            }

            return true;
        }

        false
    }
}

struct LoadedHistoryCommit {
    row: Commit,
    message: String,
    author_search_text: String,
}

fn reflog_lines_rev(
    platform: &mut gix::refs::file::log::iter::Platform<'_, '_>,
    context: &str,
    limit: Option<usize>,
) -> Result<Vec<gix::refs::log::Line>> {
    if limit == Some(0) {
        return Ok(Vec::new());
    }

    let Some(iter) = platform
        .rev()
        .map_err(|e| Error::new(ErrorKind::Backend(format!("gix reflog {context}: {e}"))))?
    else {
        return Ok(Vec::new());
    };

    let mut lines = Vec::new();
    for line in iter {
        let line =
            line.map_err(|e| Error::new(ErrorKind::Backend(format!("gix reflog {context}: {e}"))))?;
        lines.push(line);
        if let Some(limit) = limit
            && lines.len() >= limit
        {
            break;
        }
    }
    Ok(lines)
}

fn stash_reflog_lines(
    repo: &gix::Repository,
    limit: Option<usize>,
) -> Result<Vec<gix::refs::log::Line>> {
    let Some(reference) = repo.try_find_reference("refs/stash").map_err(|e| {
        Error::new(ErrorKind::Backend(format!(
            "gix try_find_reference refs/stash: {e}"
        )))
    })?
    else {
        return Ok(Vec::new());
    };

    let mut platform = reference.log_iter();
    reflog_lines_rev(&mut platform, "refs/stash", limit)
}

pub(super) fn stash_reflog_entries(repo: &gix::Repository) -> Result<Vec<StashEntry>> {
    stash_reflog_lines(repo, None)?
        .into_iter()
        .enumerate()
        .filter(|(_, line)| !line.new_oid.is_null())
        .map(|(index, line)| {
            let created_at = unix_seconds_to_system_time(line.signature.time.seconds);
            Ok(StashEntry {
                index,
                id: CommitId(oid_to_arc_str(&line.new_oid)),
                message: bstr_to_arc_str(line.message.as_ref()),
                created_at,
            })
        })
        .collect()
}

pub(super) fn stash_reflog_tips(
    repo: &gix::Repository,
    limit: usize,
) -> Result<Vec<gix::ObjectId>> {
    let mut tips = Vec::new();
    let mut seen = HashSet::default();
    for line in stash_reflog_lines(repo, Some(limit))? {
        let id = line.new_oid;
        if !id.is_null() && seen.insert(id) {
            tips.push(id);
        }
    }
    Ok(tips)
}

fn reference_commit_id(mut reference: gix::Reference<'_>) -> Result<Option<gix::ObjectId>> {
    let ref_name = reference.name().as_bstr().to_str_lossy().into_owned();
    match reference.peel_to_commit() {
        Ok(commit) => Ok(Some(commit.id().detach())),
        Err(gix::reference::peel::to_kind::Error::PeelObject(
            gix::object::peel::to_kind::Error::NotFound { .. },
        )) => Ok(None),
        Err(e) => Err(Error::new(ErrorKind::Backend(format!(
            "gix peel commit ref {ref_name}: {e}"
        )))),
    }
}

fn load_history_commit_from_walk_info(
    info: &gix::revision::walk::Info<'_>,
    decode_state: &mut CommitDecodeState,
) -> Result<LoadedHistoryCommit> {
    let id = info.id();
    let commit = id
        .repo
        .objects
        .find_commit(id.as_ref(), &mut decode_state.decode_buf)
        .map_err(|e| Error::new(ErrorKind::Backend(format!("gix commit object: {e}"))))?;

    let summary_bytes = commit.message.lines().next().unwrap_or_default();
    let summary = bstr_to_arc_str(summary_bytes);

    let (author, author_search_text) = match commit.author() {
        Ok(signature) => {
            let author = decode_state.author_cache.intern(signature.name.as_ref());
            let email = bytes_to_text_preserving_utf8(signature.email.as_ref());
            let author_search_text = if email.is_empty() {
                author.to_string()
            } else {
                format!("{author} <{email}>")
            };
            (author, author_search_text)
        }
        Err(_) => (Arc::from("unknown"), "unknown".to_string()),
    };

    let seconds = info
        .commit_time
        .unwrap_or_else(|| commit.committer().map(|t| t.seconds()).unwrap_or(0));
    let time = unix_seconds_to_system_time_or_epoch(seconds);

    let message = bytes_to_text_preserving_utf8(commit.message.as_ref())
        .trim_end()
        .to_string();

    let commit_id = decode_state
        .next_commit_id_cache
        .reuse_or_new(id.as_ref(), || CommitId(oid_to_arc_str(id.as_ref())));

    let mut parent_ids = CommitParentIds::new();
    parent_ids.reserve(info.parent_ids.len());
    if info.parent_ids.is_empty() {
        decode_state.next_commit_id_cache.clear();
    }
    for (index, parent_id) in info.parent_ids.iter().enumerate() {
        let parent_commit_id = CommitId(oid_to_arc_str(parent_id));
        if index == 0 {
            decode_state
                .next_commit_id_cache
                .remember(parent_id, &parent_commit_id);
        }
        parent_ids.push(parent_commit_id);
    }

    Ok(LoadedHistoryCommit {
        row: Commit {
            id: commit_id,
            parent_ids,
            summary,
            author,
            time,
        },
        message,
        author_search_text,
    })
}

fn commit_from_walk_info(
    info: &gix::revision::walk::Info<'_>,
    decode_state: &mut CommitDecodeState,
) -> Result<Commit> {
    load_history_commit_from_walk_info(info, decode_state).map(|loaded| loaded.row)
}

#[derive(Default)]
struct CommitDecodeState {
    decode_buf: Vec<u8>,
    author_cache: RepeatedAuthorCache,
    next_commit_id_cache: NextCommitIdCache,
}

#[derive(Default)]
struct RepeatedAuthorCache {
    raw_name: Vec<u8>,
    value: Option<Arc<str>>,
}

impl RepeatedAuthorCache {
    fn intern(&mut self, name: &[u8]) -> Arc<str> {
        if let Some(value) = self.value.as_ref()
            && self.raw_name.as_slice() == name
        {
            return Arc::clone(value);
        }

        self.raw_name.clear();
        self.raw_name.extend_from_slice(name);
        let value = bstr_to_arc_str(name);
        self.value = Some(Arc::clone(&value));
        value
    }
}

#[derive(Default)]
struct NextCommitIdCache {
    raw_id: Vec<u8>,
    value: Option<CommitId>,
}

impl NextCommitIdCache {
    fn reuse_or_new(&self, oid: &gix::oid, make: impl FnOnce() -> CommitId) -> CommitId {
        if let Some(value) = self.value.as_ref()
            && self.raw_id.as_slice() == oid.as_bytes()
        {
            return value.clone();
        }
        make()
    }

    fn remember(&mut self, oid: &gix::oid, value: &CommitId) {
        self.raw_id.clear();
        self.raw_id.extend_from_slice(oid.as_bytes());
        self.value = Some(value.clone());
    }

    fn clear(&mut self) {
        self.raw_id.clear();
        self.value = None;
    }
}

fn commit_file_change_from_diff(
    change: gix::object::tree::diff::ChangeDetached,
) -> Result<Option<CommitFileChange>> {
    use gitcomet_core::domain::FileStatusKind;
    use gix::object::tree::diff::ChangeDetached;

    let (location, is_tree, kind) = match change {
        ChangeDetached::Addition {
            entry_mode,
            location,
            ..
        } => (location, entry_mode.is_tree(), FileStatusKind::Added),
        ChangeDetached::Deletion {
            entry_mode,
            location,
            ..
        } => (location, entry_mode.is_tree(), FileStatusKind::Deleted),
        ChangeDetached::Modification {
            previous_entry_mode,
            entry_mode,
            location,
            ..
        } => (
            location,
            previous_entry_mode.is_tree() || entry_mode.is_tree(),
            FileStatusKind::Modified,
        ),
        ChangeDetached::Rewrite {
            source_entry_mode,
            entry_mode,
            location,
            copy,
            ..
        } => (
            location,
            source_entry_mode.is_tree() || entry_mode.is_tree(),
            if copy {
                FileStatusKind::Added
            } else {
                FileStatusKind::Renamed
            },
        ),
    };

    if is_tree {
        return Ok(None);
    }
    Ok(Some(CommitFileChange {
        path: path_buf_from_git_bytes(location.as_ref(), "gix commit details diff path")?,
        kind,
    }))
}

fn commit_file_changes(
    repo: &gix::Repository,
    commit: &gix::Commit<'_>,
    parent_ids: &[gix::ObjectId],
) -> Result<Vec<CommitFileChange>> {
    if parent_ids.len() > 1 {
        return Ok(Vec::new());
    }

    let commit_tree = commit
        .tree()
        .map_err(|e| Error::new(ErrorKind::Backend(format!("gix commit tree: {e}"))))?;
    let parent_tree = parent_ids
        .first()
        .map(|&id| {
            repo.find_commit(id)
                .map_err(|e| Error::new(ErrorKind::Backend(format!("gix parent commit: {e}"))))?
                .tree()
                .map_err(|e| Error::new(ErrorKind::Backend(format!("gix parent tree: {e}"))))
        })
        .transpose()?;
    let changes = repo
        .diff_tree_to_tree(parent_tree.as_ref(), &commit_tree, None)
        .map_err(|e| Error::new(ErrorKind::Backend(format!("gix diff_tree_to_tree: {e}"))))?;

    changes
        .into_iter()
        .filter_map(|change| commit_file_change_from_diff(change).transpose())
        .collect()
}

fn empty_log_page() -> LogPage {
    LogPage {
        commits: Vec::new(),
        next_cursor: None,
    }
}

fn history_query_needs_refs(query: &HistoryQuery) -> bool {
    query.has_plain_terms() || query.uses_field(HistoryQueryField::Ref)
}

fn history_ref_label(full_name: &str) -> String {
    full_name
        .strip_prefix("refs/heads/")
        .or_else(|| full_name.strip_prefix("refs/remotes/"))
        .or_else(|| full_name.strip_prefix("refs/tags/"))
        .or_else(|| full_name.strip_prefix("refs/"))
        .unwrap_or(full_name)
        .to_string()
}

fn history_refs_text_by_target(repo: &gix::Repository) -> Result<HashMap<gix::ObjectId, Arc<str>>> {
    let mut names_by_target: HashMap<gix::ObjectId, Vec<String>> = HashMap::default();

    if let Some(head_id) = gix_head_id_or_none(repo)?
        && let Some(head_name) = repo.head_name().ok().flatten()
    {
        let head_name = head_name.as_bstr().to_str_lossy();
        let labels = names_by_target.entry(head_id).or_default();
        labels.push("HEAD".to_string());
        let short = history_ref_label(head_name.as_ref());
        if short != "HEAD" {
            labels.push(short);
        }
    }

    let refs = repo
        .references()
        .map_err(|e| Error::new(ErrorKind::Backend(format!("gix references: {e}"))))?;
    let iter = refs
        .all()
        .map_err(|e| Error::new(ErrorKind::Backend(format!("gix references(all): {e}"))))?;
    for reference in iter {
        let reference =
            reference.map_err(|e| Error::new(ErrorKind::Backend(format!("gix ref iter: {e}"))))?;
        let full_name = reference.name().as_bstr().to_str_lossy().into_owned();
        if !full_name.starts_with("refs/") {
            continue;
        }
        let Some(target) = reference_commit_id(reference)? else {
            continue;
        };
        names_by_target
            .entry(target)
            .or_default()
            .push(history_ref_label(&full_name));
    }

    let mut refs_text_by_target = HashMap::default();
    for (target, mut labels) in names_by_target {
        labels.sort();
        labels.dedup();
        if labels.is_empty() {
            continue;
        }
        refs_text_by_target.insert(target, Arc::from(labels.join(" ")));
    }
    Ok(refs_text_by_target)
}

fn object_id_from_commit_id(id: &CommitId) -> Option<gix::ObjectId> {
    gix::ObjectId::from_hex(id.as_ref().as_bytes()).ok()
}

fn apply_first_parent_resume_hint(page: &mut LogPage) {
    if let Some(cursor) = page.next_cursor.as_mut() {
        cursor.resume_from = page
            .commits
            .last()
            .and_then(|commit| commit.parent_ids.first().cloned());
    }
}

fn reflog_unborn_head_error(repo: &gix::Repository) -> Error {
    let branch = repo
        .head_name()
        .ok()
        .flatten()
        .map(|name| {
            let name = name.as_bstr().to_str_lossy();
            name.strip_prefix("refs/heads/")
                .unwrap_or(name.as_ref())
                .to_string()
        })
        .unwrap_or_else(|| "HEAD".to_string());
    let detail = format!("fatal: your current branch '{branch}' does not have any commits yet");
    let stderr = format!("{detail}\n").into_bytes();
    Error::new(ErrorKind::Git(GitFailure::new(
        "git reflog",
        GitFailureId::CommandFailed,
        Some(128),
        Vec::new(),
        stderr,
        Some(detail),
    )))
}

fn paginate_commits(
    commits: impl Iterator<Item = Result<Commit>>,
    limit: usize,
    cursor: Option<&LogCursor>,
) -> Result<LogPage> {
    if limit == 0 {
        return Ok(empty_log_page());
    }

    let mut cursor_gate = CursorGate::new(cursor);
    let mut result: Vec<Commit> = Vec::with_capacity(limit);
    let mut next_cursor: Option<LogCursor> = None;

    for commit in commits {
        let commit = commit?;
        if cursor_gate.should_skip(commit.id.as_ref()) {
            continue;
        }

        if result.len() >= limit {
            next_cursor = result.last().map(|c| LogCursor {
                last_seen: c.id.clone(),
                resume_from: None,
            });
            break;
        }

        result.push(commit);
    }

    Ok(LogPage {
        commits: result,
        next_cursor,
    })
}

fn log_page_from_walk<'repo, E>(
    walk: impl Iterator<Item = std::result::Result<gix::revision::walk::Info<'repo>, E>>,
    limit: usize,
    cursor: Option<&LogCursor>,
) -> Result<LogPage>
where
    E: std::fmt::Display,
{
    let mut decode_state = CommitDecodeState::default();
    let mut cursor_gate = CursorGate::new(cursor);
    let mut commits: Vec<Commit> = Vec::with_capacity(limit);
    let mut next_cursor = None;

    for result in walk {
        let info = result.map_err(|e| Error::new(ErrorKind::Backend(format!("gix walk: {e}"))))?;
        if cursor_gate.should_skip_oid(info.id().as_ref()) {
            continue;
        }

        if commits.len() >= limit {
            next_cursor = commits.last().map(|commit| LogCursor {
                last_seen: commit.id.clone(),
                resume_from: None,
            });
            break;
        }

        commits.push(commit_from_walk_info(&info, &mut decode_state)?);
    }

    Ok(LogPage {
        commits,
        next_cursor,
    })
}

fn log_page_from_filtered_walk<'repo, E>(
    repo: &GixRepo,
    walk: impl Iterator<Item = std::result::Result<gix::revision::walk::Info<'repo>, E>>,
    limit: usize,
    cursor: Option<&LogCursor>,
    query: &HistoryQuery,
    refs_text_by_target: Option<&HashMap<gix::ObjectId, Arc<str>>>,
) -> Result<LogPage>
where
    E: std::fmt::Display,
{
    let mut decode_state = CommitDecodeState::default();
    let mut cursor_gate = CursorGate::new(cursor);
    let mut commits: Vec<Commit> = Vec::with_capacity(limit);
    let mut next_cursor = None;

    for result in walk {
        let info = result.map_err(|e| Error::new(ErrorKind::Backend(format!("gix walk: {e}"))))?;
        if cursor_gate.should_skip_oid(info.id().as_ref()) {
            continue;
        }

        let loaded = load_history_commit_from_walk_info(&info, &mut decode_state)?;
        let refs_text = refs_text_by_target
            .and_then(|refs| refs.get(&info.id().detach()))
            .map(|text| text.as_ref());
        if !repo.history_query_matches_loaded_commit(query, &loaded, refs_text)? {
            continue;
        }

        if commits.len() >= limit {
            next_cursor = commits.last().map(|commit| LogCursor {
                last_seen: commit.id.clone(),
                resume_from: None,
            });
            break;
        }

        commits.push(loaded.row);
    }

    Ok(LogPage {
        commits,
        next_cursor,
    })
}

impl GixRepo {
    fn history_query_matches_loaded_commit(
        &self,
        query: &HistoryQuery,
        commit: &LoadedHistoryCommit,
        refs_text: Option<&str>,
    ) -> Result<bool> {
        if query.is_empty() {
            return Ok(true);
        }

        let mut plain_text: Option<String> = None;
        let mut file_paths_text: Option<String> = None;
        let mut patch_text: Option<String> = None;

        for term in query.terms() {
            let needle = match AsciiCaseInsensitiveNeedle::new(term.value()) {
                Some(needle) => needle,
                None => continue,
            };

            let matched = match term {
                HistoryQueryTerm::Plain(_) => {
                    let text = plain_text.get_or_insert_with(|| {
                        let mut text = String::with_capacity(
                            commit.message.len()
                                + commit.author_search_text.len()
                                + refs_text.map_or(0, str::len)
                                + 4,
                        );
                        text.push_str(&commit.message);
                        text.push('\n');
                        text.push_str(&commit.author_search_text);
                        if let Some(refs_text) = refs_text
                            && !refs_text.is_empty()
                        {
                            text.push('\n');
                            text.push_str(refs_text);
                        }
                        text
                    });
                    needle.is_match(text)
                }
                HistoryQueryTerm::Field { field, .. } => match field {
                    HistoryQueryField::Message => needle.is_match(&commit.message),
                    HistoryQueryField::Author => needle.is_match(&commit.author_search_text),
                    HistoryQueryField::Ref => refs_text.is_some_and(|text| needle.is_match(text)),
                    HistoryQueryField::Sha => needle.is_match(commit.row.id.as_ref()),
                    HistoryQueryField::File => {
                        let text = if let Some(text) = file_paths_text.as_ref() {
                            text
                        } else {
                            file_paths_text =
                                Some(self.commit_changed_paths_text(commit.row.id.as_ref())?);
                            file_paths_text.as_ref().expect("file paths text")
                        };
                        needle.is_match(text)
                    }
                    HistoryQueryField::Content => {
                        let text = if let Some(text) = patch_text.as_ref() {
                            text
                        } else {
                            patch_text = Some(self.commit_patch_text(commit.row.id.as_ref())?);
                            patch_text.as_ref().expect("patch text")
                        };
                        needle.is_match(text)
                    }
                },
            };

            if !matched {
                return Ok(false);
            }
        }

        Ok(true)
    }

    fn commit_changed_paths_text(&self, commit_id: &str) -> Result<String> {
        let mut cmd = self.git_workdir_cmd();
        cmd.arg("-c")
            .arg("color.ui=false")
            .arg("--no-pager")
            .arg("show")
            .arg("--format=")
            .arg("--name-status")
            .arg("--find-renames=50%")
            .arg(commit_id);
        run_git_capture(cmd, "git show --name-status")
    }

    fn commit_patch_text(&self, commit_id: &str) -> Result<String> {
        let mut cmd = self.git_workdir_cmd();
        cmd.arg("-c")
            .arg("color.ui=false")
            .arg("--no-pager")
            .arg("show")
            .arg("--format=")
            .arg("--no-ext-diff")
            .arg("--find-renames=50%")
            .arg(commit_id);
        run_git_capture(cmd, "git show")
    }

    fn log_head_page_cache_key(
        head_oid: Option<gix::ObjectId>,
        limit: usize,
        cursor: Option<&LogCursor>,
    ) -> super::LogHeadPageCacheKey {
        super::LogHeadPageCacheKey {
            head_oid,
            limit,
            last_seen: cursor.map(|cursor| cursor.last_seen.clone()),
            resume_from: cursor.and_then(|cursor| cursor.resume_from.clone()),
        }
    }

    fn cached_log_head_page(&self, key: &super::LogHeadPageCacheKey) -> Option<LogPage> {
        let mut cache = self
            .log_head_page_cache
            .lock()
            .expect("log head page cache");
        let index = cache.iter().position(|entry| &entry.key == key)?;
        let entry = cache.remove(index);
        let page = entry.page.clone();
        cache.push(entry);
        Some(page)
    }

    fn store_log_head_page(&self, key: super::LogHeadPageCacheKey, page: &LogPage) {
        let mut cache = self
            .log_head_page_cache
            .lock()
            .expect("log head page cache");
        if let Some(index) = cache.iter().position(|entry| entry.key == key) {
            cache.remove(index);
        }
        if cache.len() >= super::LOG_HEAD_PAGE_CACHE_LIMIT {
            cache.remove(0);
        }
        cache.push(super::LogHeadPageCacheEntry {
            key,
            page: page.clone(),
        });
    }

    fn log_follow_commits(&self, path: &Path, max_count: Option<usize>) -> Result<Vec<Commit>> {
        let mut cmd = self.git_workdir_cmd();
        cmd.arg("log")
            .arg("--follow")
            .arg("--date=unix")
            .arg("--pretty=format:%H%x1f%P%x1f%an%x1f%ct%x1f%s%x1e");
        if let Some(max_count) = max_count {
            cmd.arg(format!("-n{max_count}"));
        }
        cmd.arg("--").arg(path);

        let output = run_git_capture(cmd, "git log --follow")?;
        Ok(parse_git_log_pretty_records(&output).commits)
    }

    pub(super) fn log_head_page_impl(
        &self,
        limit: usize,
        cursor: Option<&LogCursor>,
        query: Option<&HistoryQuery>,
    ) -> Result<LogPage> {
        if limit == 0 {
            return Ok(empty_log_page());
        }

        let repo = self._repo.to_thread_local();
        if let Some(query) = query.filter(|query| !query.is_empty()) {
            let refs_text_by_target = if history_query_needs_refs(query) {
                Some(history_refs_text_by_target(&repo)?)
            } else {
                None
            };

            let page = if let Some(resume_tip) = cursor
                .and_then(|cursor| cursor.resume_from.as_ref())
                .and_then(object_id_from_commit_id)
            {
                let walk = repo
                    .rev_walk([resume_tip])
                    .sorting(gix::revision::walk::Sorting::ByCommitTime(
                        CommitTimeOrder::NewestFirst,
                    ))
                    .first_parent_only()
                    .all()
                    .map_err(|e| Error::new(ErrorKind::Backend(format!("gix rev_walk: {e}"))))?;
                let mut page = log_page_from_filtered_walk(
                    self,
                    walk,
                    limit,
                    None,
                    query,
                    refs_text_by_target.as_ref(),
                )?;
                apply_first_parent_resume_hint(&mut page);
                page
            } else if let Some(head_id) = gix_head_id_or_none(&repo)? {
                let walk = repo
                    .rev_walk([head_id])
                    .sorting(gix::revision::walk::Sorting::ByCommitTime(
                        CommitTimeOrder::NewestFirst,
                    ))
                    .first_parent_only()
                    .all()
                    .map_err(|e| Error::new(ErrorKind::Backend(format!("gix rev_walk: {e}"))))?;
                let mut page = log_page_from_filtered_walk(
                    self,
                    walk,
                    limit,
                    cursor,
                    query,
                    refs_text_by_target.as_ref(),
                )?;
                apply_first_parent_resume_hint(&mut page);
                page
            } else {
                empty_log_page()
            };

            return Ok(page);
        }

        let head_id = gix_head_id_or_none(&repo)?;
        let cache_key = Self::log_head_page_cache_key(head_id, limit, cursor);
        if let Some(page) = self.cached_log_head_page(&cache_key) {
            return Ok(page);
        }

        let page = if let Some(resume_tip) = cursor
            .and_then(|cursor| cursor.resume_from.as_ref())
            .and_then(object_id_from_commit_id)
        {
            let walk = repo
                .rev_walk([resume_tip])
                .sorting(gix::revision::walk::Sorting::ByCommitTime(
                    CommitTimeOrder::NewestFirst,
                ))
                .first_parent_only()
                .all()
                .map_err(|e| Error::new(ErrorKind::Backend(format!("gix rev_walk: {e}"))))?;
            let mut page = log_page_from_walk(walk, limit, None)?;
            apply_first_parent_resume_hint(&mut page);
            page
        } else if let Some(head_id) = head_id {
            let walk = repo
                .rev_walk([head_id])
                .sorting(gix::revision::walk::Sorting::ByCommitTime(
                    CommitTimeOrder::NewestFirst,
                ))
                .first_parent_only()
                .all()
                .map_err(|e| Error::new(ErrorKind::Backend(format!("gix rev_walk: {e}"))))?;
            let mut page = log_page_from_walk(walk, limit, cursor)?;
            apply_first_parent_resume_hint(&mut page);
            page
        } else {
            empty_log_page()
        };

        self.store_log_head_page(cache_key, &page);
        Ok(page)
    }

    pub(super) fn log_all_branches_page_impl(
        &self,
        limit: usize,
        cursor: Option<&LogCursor>,
        query: Option<&HistoryQuery>,
    ) -> Result<LogPage> {
        if limit == 0 {
            return Ok(empty_log_page());
        }

        let repo = self._repo.to_thread_local();
        let head_id = gix_head_id_or_none(&repo)?;

        let refs = repo
            .references()
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix references: {e}"))))?;

        // Emulate `git log --all`: include all refs under `refs/`, not just `refs/heads` and
        // `refs/remotes`. Some repositories (e.g. Chromium) use additional namespaces like
        // `refs/branch-heads/*`.
        let mut tips = Vec::new();
        let mut seen = HashSet::default();
        if let Some(head_id) = head_id {
            tips.push(head_id);
            seen.insert(head_id);
        }

        let iter = refs
            .all()
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix references(all): {e}"))))?;
        for reference in iter {
            let reference = reference
                .map_err(|e| Error::new(ErrorKind::Backend(format!("gix ref iter: {e}"))))?;
            if matches!(
                reference.name().category(),
                Some(gix::reference::Category::Tag)
            ) {
                continue;
            }
            let Some(id) = reference_commit_id(reference)? else {
                continue;
            };
            if seen.insert(id) {
                tips.push(id);
            }
        }

        // `git log --all` includes only `refs/stash` tip, but users expect history scope=all
        // to also surface older stash entries (reflog-backed). Add stash reflog commits as extra
        // walk tips so stash rows can be rendered consistently in history graph.
        for id in stash_reflog_tips(&repo, 50).unwrap_or_default() {
            if seen.insert(id) {
                tips.push(id);
            }
        }

        if tips.is_empty() {
            return Ok(empty_log_page());
        }

        let walk = repo
            .rev_walk(tips)
            .sorting(gix::revision::walk::Sorting::ByCommitTime(
                CommitTimeOrder::NewestFirst,
            ))
            .all()
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix rev_walk: {e}"))))?;
        if let Some(query) = query.filter(|query| !query.is_empty()) {
            let refs_text_by_target = if history_query_needs_refs(query) {
                Some(history_refs_text_by_target(&repo)?)
            } else {
                None
            };
            log_page_from_filtered_walk(
                self,
                walk,
                limit,
                cursor,
                query,
                refs_text_by_target.as_ref(),
            )
        } else {
            log_page_from_walk(walk, limit, cursor)
        }
    }

    pub(super) fn log_file_page_impl(
        &self,
        path: &Path,
        limit: usize,
        cursor: Option<&LogCursor>,
        _query: Option<&HistoryQuery>,
    ) -> Result<LogPage> {
        if limit == 0 {
            return Ok(empty_log_page());
        }

        // Only the first page is bounded. `git log --follow` does not combine
        // reliably with `--skip` across renames, so cursor pages still need to
        // scan the full follow history and paginate it in-process.
        let max_count = cursor.is_none().then_some(limit.saturating_add(1));
        let commits = self.log_follow_commits(path, max_count)?;
        paginate_commits(commits.into_iter().map(Ok), limit, cursor)
    }

    pub(super) fn commit_details_impl(&self, id: &CommitId) -> Result<CommitDetails> {
        let repo = self._repo.to_thread_local();
        let spec = id.as_ref();
        let commit = repo
            .rev_parse_single(spec)
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix rev-parse {spec}: {e}"))))?
            .object()
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix commit object {spec}: {e}"))))?
            .peel_to_commit()
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix peel commit {spec}: {e}"))))?;

        let message = bytes_to_text_preserving_utf8(commit.message_raw_sloppy().as_ref())
            .trim_end()
            .to_string();
        let committed_at = commit
            .time()
            .map(|time| time.format_or_unix(gix::date::time::format::ISO8601_STRICT))
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix commit time {spec}: {e}"))))?;
        let parent_oids = commit
            .parent_ids()
            .map(|parent| parent.detach())
            .collect::<Vec<_>>();
        let parent_ids = parent_oids
            .iter()
            .map(|parent| CommitId(oid_to_arc_str(parent)))
            .collect::<Vec<_>>();
        let files = commit_file_changes(&repo, &commit, &parent_oids)?;

        Ok(CommitDetails {
            id: id.clone(),
            message,
            committed_at,
            parent_ids,
            files,
        })
    }

    pub(super) fn reflog_head_impl(&self, limit: usize) -> Result<Vec<ReflogEntry>> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let repo = self._repo.to_thread_local();
        if gix_head_id_or_none(&repo)?.is_none() {
            return Err(reflog_unborn_head_error(&repo));
        }

        let head = repo
            .head()
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix head: {e}"))))?;
        let mut platform = head.log_iter();
        reflog_lines_rev(&mut platform, "HEAD", Some(limit))?
            .into_iter()
            .enumerate()
            .map(|(index, line)| {
                Ok(ReflogEntry {
                    index,
                    new_id: CommitId(oid_to_arc_str(&line.new_oid)),
                    message: bstr_to_arc_str(line.message.as_ref()),
                    time: unix_seconds_to_system_time(line.signature.time.seconds),
                    selector: format!("HEAD@{{{index}}}").into(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_gate_skips_until_after_last_seen() {
        let cursor = LogCursor {
            last_seen: CommitId("c2".into()),
            resume_from: None,
        };
        let mut gate = CursorGate::new(Some(&cursor));

        assert!(gate.should_skip("c1"));
        assert!(gate.should_skip("c2"));
        assert!(!gate.should_skip("c3"));
        assert!(!gate.should_skip("c4"));
    }

    #[test]
    fn object_id_from_commit_id_rejects_invalid_hex() {
        assert!(object_id_from_commit_id(&CommitId("not-a-sha".into())).is_none());
    }

    #[test]
    fn apply_first_parent_resume_hint_uses_first_parent_of_last_commit() {
        let mut page = LogPage {
            commits: vec![
                Commit {
                    id: CommitId("c1".into()),
                    parent_ids: CommitParentIds::from_vec(vec![CommitId("p0".into())]),
                    summary: Arc::from("one"),
                    author: Arc::from("you"),
                    time: std::time::SystemTime::UNIX_EPOCH,
                },
                Commit {
                    id: CommitId("c2".into()),
                    parent_ids: CommitParentIds::from_vec(vec![
                        CommitId("p1".into()),
                        CommitId("p2".into()),
                    ]),
                    summary: Arc::from("two"),
                    author: Arc::from("you"),
                    time: std::time::SystemTime::UNIX_EPOCH,
                },
            ],
            next_cursor: Some(LogCursor {
                last_seen: CommitId("c2".into()),
                resume_from: None,
            }),
        };

        apply_first_parent_resume_hint(&mut page);

        assert_eq!(
            page.next_cursor
                .as_ref()
                .and_then(|cursor| cursor.resume_from.clone()),
            Some(CommitId("p1".into()))
        );
    }

    #[test]
    fn apply_first_parent_resume_hint_clears_stale_resume_hint_when_no_parent_exists() {
        let mut page = LogPage {
            commits: vec![Commit {
                id: CommitId("c1".into()),
                parent_ids: CommitParentIds::new(),
                summary: Arc::from("one"),
                author: Arc::from("you"),
                time: std::time::SystemTime::UNIX_EPOCH,
            }],
            next_cursor: Some(LogCursor {
                last_seen: CommitId("c1".into()),
                resume_from: Some(CommitId("stale".into())),
            }),
        };

        apply_first_parent_resume_hint(&mut page);

        assert_eq!(
            page.next_cursor.as_ref().expect("next cursor").resume_from,
            None
        );
    }

    #[test]
    fn repeated_author_cache_reuses_arc_for_identical_names() {
        let mut cache = RepeatedAuthorCache::default();

        let first = cache.intern(b"Bench");
        let second = cache.intern(b"Bench");
        let third = cache.intern(b"Other");

        assert!(Arc::ptr_eq(&first, &second));
        assert!(!Arc::ptr_eq(&second, &third));
    }

    #[test]
    fn next_commit_id_cache_reuses_commit_id_for_matching_first_parent() {
        let mut cache = NextCommitIdCache::default();

        let parent = CommitId(Arc::from("1111111111111111111111111111111111111111"));
        let oid = gix::ObjectId::from_hex(parent.as_ref().as_bytes()).expect("valid oid");
        cache.remember(oid.as_ref(), &parent);

        let reused = cache.reuse_or_new(oid.as_ref(), || CommitId(Arc::from("other")));
        let other_oid = gix::ObjectId::from_hex(b"2222222222222222222222222222222222222222")
            .expect("valid oid");
        let fresh = cache.reuse_or_new(other_oid.as_ref(), || CommitId(Arc::from("fresh")));

        assert!(Arc::ptr_eq(&parent.0, &reused.0));
        assert_eq!(fresh.as_ref(), "fresh");
    }
}
