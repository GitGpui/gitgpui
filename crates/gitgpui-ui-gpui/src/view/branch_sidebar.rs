use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BranchSection {
    Local,
    Remote,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum BranchSidebarRow {
    SectionHeader {
        section: BranchSection,
        top_border: bool,
    },
    SectionSpacer,
    Placeholder {
        section: BranchSection,
        message: SharedString,
    },
    RemoteHeader {
        name: SharedString,
    },
    GroupHeader {
        label: SharedString,
        depth: usize,
    },
    Branch {
        label: SharedString,
        name: SharedString,
        section: BranchSection,
        depth: usize,
        muted: bool,
        divergence: Option<UpstreamDivergence>,
        divergence_ahead: Option<SharedString>,
        divergence_behind: Option<SharedString>,
        tooltip: SharedString,
        is_head: bool,
        is_upstream: bool,
    },
    WorktreesHeader {
        top_border: bool,
    },
    WorktreePlaceholder {
        message: SharedString,
    },
    WorktreeItem {
        path: std::path::PathBuf,
        label: SharedString,
        tooltip: SharedString,
        is_active: bool,
    },
    SubmodulesHeader {
        top_border: bool,
    },
    SubmodulePlaceholder {
        message: SharedString,
    },
    SubmoduleItem {
        path: std::path::PathBuf,
        label: SharedString,
        tooltip: SharedString,
    },
    StashHeader {
        top_border: bool,
    },
    StashPlaceholder {
        message: SharedString,
    },
    StashItem {
        index: usize,
        message: SharedString,
        tooltip: SharedString,
        row_group: SharedString,
        apply_button_id: SharedString,
        pop_button_id: SharedString,
        drop_button_id: SharedString,
        created_at: Option<std::time::SystemTime>,
    },
}

#[derive(Default)]
struct SlashTree {
    is_leaf: bool,
    children: BTreeMap<String, SlashTree>,
}

impl SlashTree {
    fn insert(&mut self, name: &str) {
        let mut node = self;
        for part in name.split('/').filter(|p| !p.is_empty()) {
            node = node.children.entry(part.to_string()).or_default();
        }
        node.is_leaf = true;
    }
}

pub(super) fn branch_sidebar_rows(repo: &RepoState) -> Vec<BranchSidebarRow> {
    let mut rows = Vec::new();
    let head_upstream_full = match (&repo.branches, &repo.head_branch) {
        (Loadable::Ready(branches), Loadable::Ready(head)) => branches
            .iter()
            .find(|b| b.name == *head)
            .and_then(|b| b.upstream.as_ref())
            .map(|u| format!("{}/{}", u.remote, u.branch)),
        _ => None,
    };

    rows.push(BranchSidebarRow::SectionHeader {
        section: BranchSection::Local,
        top_border: false,
    });

    match &repo.branches {
        Loadable::Ready(branches) if branches.is_empty() => {
            rows.push(BranchSidebarRow::Placeholder {
                section: BranchSection::Local,
                message: "No branches".into(),
            });
        }
        Loadable::Ready(branches) => {
            let head = match &repo.head_branch {
                Loadable::Ready(h) => Some(h.as_str()),
                _ => None,
            };
            let mut local_meta: HashMap<String, (Option<UpstreamDivergence>, bool)> =
                HashMap::default();
            for b in branches {
                local_meta.insert(
                    b.name.clone(),
                    (b.divergence, head.is_some_and(|h| h == b.name)),
                );
            }

            let mut tree = SlashTree::default();
            for branch in branches {
                tree.insert(&branch.name);
            }
            push_slash_tree_rows(
                &tree,
                &mut rows,
                Some(&local_meta),
                head_upstream_full.as_deref(),
                0,
                false,
                BranchSection::Local,
                "",
            );
        }
        Loadable::Loading => rows.push(BranchSidebarRow::Placeholder {
            section: BranchSection::Local,
            message: "Loading".into(),
        }),
        Loadable::NotLoaded => rows.push(BranchSidebarRow::Placeholder {
            section: BranchSection::Local,
            message: "Not loaded".into(),
        }),
        Loadable::Error(e) => rows.push(BranchSidebarRow::Placeholder {
            section: BranchSection::Local,
            message: e.clone().into(),
        }),
    }

    rows.push(BranchSidebarRow::SectionSpacer);
    rows.push(BranchSidebarRow::SectionHeader {
        section: BranchSection::Remote,
        top_border: true,
    });

    let mut remotes: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut remote_section_is_loading_or_error = false;
    match &repo.remote_branches {
        Loadable::Ready(branches) => {
            for branch in branches {
                remotes
                    .entry(branch.remote.clone())
                    .or_default()
                    .push(branch.name.clone());
            }
        }
        Loadable::Loading => {
            rows.push(BranchSidebarRow::Placeholder {
                section: BranchSection::Remote,
                message: "Loading".into(),
            });
            remote_section_is_loading_or_error = true;
        }
        Loadable::Error(e) => {
            rows.push(BranchSidebarRow::Placeholder {
                section: BranchSection::Remote,
                message: e.clone().into(),
            });
            remote_section_is_loading_or_error = true;
        }
        Loadable::NotLoaded => {
            if let Loadable::Ready(known) = &repo.remotes {
                for remote in known {
                    remotes.entry(remote.name.clone()).or_default();
                }
            }
        }
    }

    if !remote_section_is_loading_or_error {
        if remotes.is_empty() {
            rows.push(BranchSidebarRow::Placeholder {
                section: BranchSection::Remote,
                message: "No remotes".into(),
            });
        } else {
            for (remote, mut branches) in remotes {
                branches.sort();
                branches.dedup();
                rows.push(BranchSidebarRow::RemoteHeader {
                    name: remote.clone().into(),
                });
                if branches.is_empty() {
                    continue;
                }

                let mut tree = SlashTree::default();
                for branch in branches {
                    tree.insert(&branch);
                }
                let name_prefix = format!("{remote}/");
                push_slash_tree_rows(
                    &tree,
                    &mut rows,
                    None,
                    head_upstream_full.as_deref(),
                    1,
                    true,
                    BranchSection::Remote,
                    &name_prefix,
                );
            }
        }
    }

    rows.push(BranchSidebarRow::SectionSpacer);
    rows.push(BranchSidebarRow::WorktreesHeader { top_border: true });

    match &repo.worktrees {
        Loadable::Ready(worktrees) => {
            let mut any = false;
            for worktree in worktrees {
                any = true;
                let label: SharedString = if let Some(branch) = &worktree.branch {
                    format!("{branch}  {}", worktree.path.display()).into()
                } else if worktree.detached {
                    format!("(detached)  {}", worktree.path.display()).into()
                } else {
                    worktree.path.display().to_string().into()
                };
                let tooltip: SharedString = worktree.path.display().to_string().into();
                rows.push(BranchSidebarRow::WorktreeItem {
                    path: worktree.path.clone(),
                    label,
                    tooltip,
                    is_active: worktree.path == repo.spec.workdir,
                });
            }
            if !any {
                rows.push(BranchSidebarRow::WorktreePlaceholder {
                    message: "No worktrees".into(),
                });
            }
        }
        Loadable::Loading => rows.push(BranchSidebarRow::WorktreePlaceholder {
            message: "Loading".into(),
        }),
        Loadable::NotLoaded => rows.push(BranchSidebarRow::WorktreePlaceholder {
            message: "Not loaded".into(),
        }),
        Loadable::Error(e) => rows.push(BranchSidebarRow::WorktreePlaceholder {
            message: e.clone().into(),
        }),
    }

    rows.push(BranchSidebarRow::SectionSpacer);
    rows.push(BranchSidebarRow::SubmodulesHeader { top_border: true });

    match &repo.submodules {
        Loadable::Ready(submodules) if submodules.is_empty() => {
            rows.push(BranchSidebarRow::SubmodulePlaceholder {
                message: "No submodules".into(),
            });
        }
        Loadable::Ready(submodules) => {
            for submodule in submodules {
                let label: SharedString = submodule.path.display().to_string().into();
                let tooltip: SharedString = submodule.path.display().to_string().into();
                rows.push(BranchSidebarRow::SubmoduleItem {
                    path: submodule.path.clone(),
                    label,
                    tooltip,
                });
            }
        }
        Loadable::Loading => rows.push(BranchSidebarRow::SubmodulePlaceholder {
            message: "Loading".into(),
        }),
        Loadable::NotLoaded => rows.push(BranchSidebarRow::SubmodulePlaceholder {
            message: "Not loaded".into(),
        }),
        Loadable::Error(e) => rows.push(BranchSidebarRow::SubmodulePlaceholder {
            message: e.clone().into(),
        }),
    }

    rows.push(BranchSidebarRow::SectionSpacer);
    rows.push(BranchSidebarRow::StashHeader { top_border: true });
    match &repo.stashes {
        Loadable::Ready(stashes) if stashes.is_empty() => {
            rows.push(BranchSidebarRow::StashPlaceholder {
                message: "No stashes".into(),
            });
        }
        Loadable::Ready(stashes) => {
            for stash in stashes {
                let row_group: SharedString = format!("stash_row_{}", stash.index).into();
                let apply_button_id: SharedString =
                    format!("stash_sidebar_apply_{}", stash.index).into();
                let pop_button_id: SharedString =
                    format!("stash_sidebar_pop_{}", stash.index).into();
                let drop_button_id: SharedString =
                    format!("stash_sidebar_drop_{}", stash.index).into();
                let message: SharedString = stash.message.clone().into();
                let tooltip: SharedString = if stash.message.is_empty() {
                    "Stash".into()
                } else {
                    message.clone()
                };
                rows.push(BranchSidebarRow::StashItem {
                    index: stash.index,
                    message,
                    tooltip,
                    row_group,
                    apply_button_id,
                    pop_button_id,
                    drop_button_id,
                    created_at: stash.created_at,
                });
            }
        }
        Loadable::Loading => rows.push(BranchSidebarRow::StashPlaceholder {
            message: "Loading".into(),
        }),
        Loadable::NotLoaded => rows.push(BranchSidebarRow::StashPlaceholder {
            message: "Not loaded".into(),
        }),
        Loadable::Error(e) => rows.push(BranchSidebarRow::StashPlaceholder {
            message: e.clone().into(),
        }),
    }

    rows
}

fn push_slash_tree_rows(
    tree: &SlashTree,
    out: &mut Vec<BranchSidebarRow>,
    local_meta: Option<&HashMap<String, (Option<UpstreamDivergence>, bool)>>,
    upstream_full: Option<&str>,
    depth: usize,
    muted: bool,
    section: BranchSection,
    name_prefix: &str,
) {
    for (label, node) in &tree.children {
        if node.children.is_empty() {
            if node.is_leaf {
                let full = format!("{name_prefix}{label}");
                let is_upstream = upstream_full.is_some_and(|u| u == full.as_str());
                let (divergence, is_head) = local_meta
                    .and_then(|m| m.get(&full))
                    .copied()
                    .unwrap_or((None, false));
                let divergence_ahead = divergence
                    .filter(|d| d.ahead > 0)
                    .map(|d| d.ahead.to_string().into());
                let divergence_behind = divergence
                    .filter(|d| d.behind > 0)
                    .map(|d| d.behind.to_string().into());
                let upstream_note = if is_upstream && section == BranchSection::Remote {
                    " (upstream for current branch)"
                } else {
                    ""
                };
                let tooltip: SharedString = format!("Branch: {full}{upstream_note}").into();
                out.push(BranchSidebarRow::Branch {
                    label: label.clone().into(),
                    name: full.into(),
                    section,
                    depth,
                    muted,
                    divergence,
                    divergence_ahead,
                    divergence_behind,
                    tooltip,
                    is_head,
                    is_upstream,
                });
            }
            continue;
        }

        out.push(BranchSidebarRow::GroupHeader {
            label: format!("{label}/").into(),
            depth,
        });

        if node.is_leaf {
            let full = format!("{name_prefix}{label}");
            let is_upstream = upstream_full.is_some_and(|u| u == full.as_str());
            let (divergence, is_head) = local_meta
                .and_then(|m| m.get(&full))
                .copied()
                .unwrap_or((None, false));
            let divergence_ahead = divergence
                .filter(|d| d.ahead > 0)
                .map(|d| d.ahead.to_string().into());
            let divergence_behind = divergence
                .filter(|d| d.behind > 0)
                .map(|d| d.behind.to_string().into());
            let upstream_note = if is_upstream && section == BranchSection::Remote {
                " (upstream for current branch)"
            } else {
                ""
            };
            let tooltip: SharedString = format!("Branch: {full}{upstream_note}").into();
            out.push(BranchSidebarRow::Branch {
                label: label.clone().into(),
                name: full.into(),
                section,
                depth: depth + 1,
                muted,
                divergence,
                divergence_ahead,
                divergence_behind,
                tooltip,
                is_head,
                is_upstream,
            });
        }

        let next_prefix = format!("{name_prefix}{label}/");
        push_slash_tree_rows(
            node,
            out,
            local_meta,
            upstream_full,
            depth + 1,
            muted,
            section,
            &next_prefix,
        );
    }
}
