use gitgpui_core::domain::*;
use rustc_hash::FxHashSet;
use std::time::{Duration, SystemTime};

#[test]
fn commit_id_is_hashable() {
    let mut set = FxHashSet::default();
    set.insert(CommitId("a".into()));
    set.insert(CommitId("b".into()));
    assert!(set.contains(&CommitId("a".into())));
}

#[test]
fn log_cursor_roundtrips() {
    let cursor = LogCursor {
        last_seen: CommitId("deadbeef".into()),
    };
    assert_eq!(cursor.last_seen.0, "deadbeef");
}

#[test]
fn commit_struct_is_constructible() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
    let commit = Commit {
        id: CommitId("1".into()),
        parent_ids: vec![CommitId("0".into())],
        summary: "test".into(),
        author: "me".into(),
        time: now,
    };
    assert_eq!(commit.summary, "test");
}
