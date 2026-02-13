use super::*;

#[test]
fn pull_and_push_mark_in_flight_until_command_finished() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::default();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();

    let repo_id = RepoId(1);
    let workdir = PathBuf::from("/tmp/repo");
    repos.insert(repo_id, Arc::new(DummyRepo::new("/tmp/repo")));
    state.repos.push(RepoState::new_opening(
        repo_id,
        RepoSpec {
            workdir: workdir.clone(),
        },
    ));

    reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::Pull {
            repo_id,
            mode: PullMode::Default,
        },
    );
    assert_eq!(state.repos[0].pull_in_flight, 1);

    reduce(&mut repos, &id_alloc, &mut state, Msg::FetchAll { repo_id });
    assert_eq!(state.repos[0].pull_in_flight, 2);

    reduce(&mut repos, &id_alloc, &mut state, Msg::Push { repo_id });
    assert_eq!(state.repos[0].push_in_flight, 1);

    reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::RepoCommandFinished {
            repo_id,
            command: RepoCommandKind::FetchAll,
            result: Ok(CommandOutput::empty_success("git fetch --all")),
        },
    );
    assert_eq!(state.repos[0].pull_in_flight, 1);

    reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::RepoCommandFinished {
            repo_id,
            command: RepoCommandKind::Pull {
                mode: PullMode::Default,
            },
            result: Ok(CommandOutput::empty_success("git pull")),
        },
    );
    assert_eq!(state.repos[0].pull_in_flight, 0);

    reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::RepoCommandFinished {
            repo_id,
            command: RepoCommandKind::Push,
            result: Ok(CommandOutput::empty_success("git push")),
        },
    );
    assert_eq!(state.repos[0].push_in_flight, 0);
}

#[test]
fn pull_and_push_do_not_mark_in_flight_before_repo_is_opened() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::default();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();

    let repo_id = RepoId(1);
    state.repos.push(RepoState::new_opening(
        repo_id,
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));

    reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::Pull {
            repo_id,
            mode: PullMode::Default,
        },
    );
    reduce(&mut repos, &id_alloc, &mut state, Msg::FetchAll { repo_id });
    reduce(&mut repos, &id_alloc, &mut state, Msg::Push { repo_id });

    assert_eq!(state.repos[0].pull_in_flight, 0);
    assert_eq!(state.repos[0].push_in_flight, 0);
}

#[test]
fn commit_emits_effect() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::default();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::Commit {
            repo_id: RepoId(1),
            message: "hello".to_string(),
        },
    );

    assert!(matches!(
        effects.as_slice(),
        [Effect::Commit { repo_id: RepoId(1), message } ] if message == "hello"
    ));
}

#[test]
fn reset_emits_effect() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::default();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::Reset {
            repo_id: RepoId(1),
            target: "HEAD~1".to_string(),
            mode: gitgpui_core::services::ResetMode::Hard,
        },
    );

    assert!(matches!(
        effects.as_slice(),
        [Effect::Reset { repo_id: RepoId(1), target, mode: gitgpui_core::services::ResetMode::Hard }]
            if target == "HEAD~1"
    ));
}

#[test]
fn revert_commit_emits_effect() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::default();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::RevertCommit {
            repo_id: RepoId(1),
            commit_id: gitgpui_core::domain::CommitId("deadbeef".to_string()),
        },
    );

    assert!(matches!(
        effects.as_slice(),
        [Effect::RevertCommit {
            repo_id: RepoId(1),
            commit_id: _
        }]
    ));
}

#[test]
fn commit_amend_emits_effect() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::default();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::CommitAmend {
            repo_id: RepoId(1),
            message: "amended".to_string(),
        },
    );

    assert!(matches!(
        effects.as_slice(),
        [Effect::CommitAmend { repo_id: RepoId(1), message }] if message == "amended"
    ));
}

#[test]
fn merge_ref_emits_effect() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::default();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::MergeRef {
            repo_id: RepoId(1),
            reference: "feature".to_string(),
        },
    );

    assert!(matches!(
        effects.as_slice(),
        [Effect::MergeRef { repo_id: RepoId(1), reference }] if reference == "feature"
    ));
}

#[test]
fn rebase_emits_effect() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::default();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::Rebase {
            repo_id: RepoId(1),
            onto: "master".to_string(),
        },
    );

    assert!(matches!(
        effects.as_slice(),
        [Effect::Rebase { repo_id: RepoId(1), onto }] if onto == "master"
    ));
}

#[test]
fn create_and_delete_branch_emit_effects() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::default();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::CreateBranch {
            repo_id: RepoId(1),
            name: "feature".to_string(),
        },
    );
    assert!(matches!(
        effects.as_slice(),
        [Effect::CreateBranch { repo_id: RepoId(1), name }] if name == "feature"
    ));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::DeleteBranch {
            repo_id: RepoId(1),
            name: "feature".to_string(),
        },
    );
    assert!(matches!(
        effects.as_slice(),
        [Effect::DeleteBranch { repo_id: RepoId(1), name }] if name == "feature"
    ));
}

#[test]
fn create_and_delete_tag_emit_effects() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::default();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::CreateTag {
            repo_id: RepoId(1),
            name: "v1.0.0".to_string(),
            target: "HEAD".to_string(),
        },
    );
    assert!(matches!(
        effects.as_slice(),
        [Effect::CreateTag { repo_id: RepoId(1), name, target }] if name == "v1.0.0" && target == "HEAD"
    ));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::DeleteTag {
            repo_id: RepoId(1),
            name: "v1.0.0".to_string(),
        },
    );
    assert!(matches!(
        effects.as_slice(),
        [Effect::DeleteTag { repo_id: RepoId(1), name }] if name == "v1.0.0"
    ));
}

#[test]
fn apply_drop_and_pop_stash_emit_effects() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::default();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let apply = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::ApplyStash {
            repo_id: RepoId(1),
            index: 0,
        },
    );
    assert!(matches!(
        apply.as_slice(),
        [Effect::ApplyStash {
            repo_id: RepoId(1),
            index: 0
        }]
    ));

    let drop = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::DropStash {
            repo_id: RepoId(1),
            index: 0,
        },
    );
    assert!(matches!(
        drop.as_slice(),
        [Effect::DropStash {
            repo_id: RepoId(1),
            index: 0
        }]
    ));

    let pop = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::PopStash {
            repo_id: RepoId(1),
            index: 0,
        },
    );
    assert!(matches!(
        pop.as_slice(),
        [Effect::PopStash {
            repo_id: RepoId(1),
            index: 0
        }]
    ));
}

#[test]
fn checkout_commit_emits_effect() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::default();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::CheckoutCommit {
            repo_id: RepoId(1),
            commit_id: gitgpui_core::domain::CommitId("deadbeef".to_string()),
        },
    );

    assert!(matches!(
        effects.as_slice(),
        [Effect::CheckoutCommit {
            repo_id: RepoId(1),
            commit_id: _
        }]
    ));
}

#[test]
fn discard_worktree_changes_path_emits_effect() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::default();
    let id_alloc = AtomicU64::new(2);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::DiscardWorktreeChangesPath {
            repo_id: RepoId(1),
            path: PathBuf::from("a.txt"),
        },
    );

    assert!(matches!(
        effects.as_slice(),
        [Effect::DiscardWorktreeChangesPath {
            repo_id: RepoId(1),
            path: _
        }]
    ));
}

#[test]
fn repo_operations_emit_effects() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::default();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let stage = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::StagePath {
            repo_id: RepoId(1),
            path: PathBuf::from("a.txt"),
        },
    );
    assert!(matches!(
        stage.as_slice(),
        [Effect::StagePath {
            repo_id: RepoId(1),
            ..
        }]
    ));

    let unstage = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::UnstagePath {
            repo_id: RepoId(1),
            path: PathBuf::from("a.txt"),
        },
    );
    assert!(matches!(
        unstage.as_slice(),
        [Effect::UnstagePath {
            repo_id: RepoId(1),
            ..
        }]
    ));

    let commit = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::Commit {
            repo_id: RepoId(1),
            message: "m".to_string(),
        },
    );
    assert!(matches!(
        commit.as_slice(),
        [Effect::Commit {
            repo_id: RepoId(1),
            ..
        }]
    ));

    let pull = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::Pull {
            repo_id: RepoId(1),
            mode: PullMode::Rebase,
        },
    );
    assert!(matches!(
        pull.as_slice(),
        [Effect::Pull {
            repo_id: RepoId(1),
            ..
        }]
    ));

    let push = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::Push { repo_id: RepoId(1) },
    );
    assert!(matches!(
        push.as_slice(),
        [Effect::Push { repo_id: RepoId(1) }]
    ));

    let force_push = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::ForcePush { repo_id: RepoId(1) },
    );
    assert!(matches!(
        force_push.as_slice(),
        [Effect::ForcePush { repo_id: RepoId(1) }]
    ));

    let push_set_upstream = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::PushSetUpstream {
            repo_id: RepoId(1),
            remote: "origin".to_string(),
            branch: "feature/foo".to_string(),
        },
    );
    assert!(matches!(
        push_set_upstream.as_slice(),
        [Effect::PushSetUpstream {
            repo_id: RepoId(1),
            ..
        }]
    ));

    let stash = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::Stash {
            repo_id: RepoId(1),
            message: "wip".to_string(),
            include_untracked: true,
        },
    );
    assert!(matches!(
        stash.as_slice(),
        [Effect::Stash {
            repo_id: RepoId(1),
            ..
        }]
    ));
}
