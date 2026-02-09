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
        is_head: bool,
        is_upstream: bool,
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
            let mut local_meta: std::collections::HashMap<
                String,
                (Option<UpstreamDivergence>, bool),
            > = std::collections::HashMap::new();
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
            return rows;
        }
        Loadable::Error(e) => {
            rows.push(BranchSidebarRow::Placeholder {
                section: BranchSection::Remote,
                message: e.clone().into(),
            });
            return rows;
        }
        Loadable::NotLoaded => {
            if let Loadable::Ready(known) = &repo.remotes {
                for remote in known {
                    remotes.entry(remote.name.clone()).or_default();
                }
            }
        }
    }

    if remotes.is_empty() {
        rows.push(BranchSidebarRow::Placeholder {
            section: BranchSection::Remote,
            message: "No remotes".into(),
        });
        return rows;
    }

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
                rows.push(BranchSidebarRow::StashItem {
                    index: stash.index,
                    message: stash.message.clone().into(),
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
    local_meta: Option<&std::collections::HashMap<String, (Option<UpstreamDivergence>, bool)>>,
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
                out.push(BranchSidebarRow::Branch {
                    label: label.clone().into(),
                    name: full.into(),
                    section,
                    depth,
                    muted,
                    divergence,
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
            out.push(BranchSidebarRow::Branch {
                label: label.clone().into(),
                name: full.into(),
                section,
                depth: depth + 1,
                muted,
                divergence,
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
