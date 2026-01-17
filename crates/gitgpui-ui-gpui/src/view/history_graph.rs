use crate::theme::AppTheme;
use gitgpui_core::domain::{Commit, CommitId};
use gpui::Rgba;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct LaneId(pub u64);

#[derive(Clone, Copy, Debug)]
pub struct LanePaint {
    pub id: LaneId,
    pub color: Rgba,
}

#[derive(Clone, Copy, Debug)]
pub struct GraphEdge {
    pub from_col: usize,
    pub to_col: usize,
    pub color: Rgba,
}

#[derive(Clone, Debug)]
pub struct GraphRow {
    pub incoming_ids: Vec<LaneId>,
    pub lanes_now: Vec<LanePaint>,
    pub lanes_next: Vec<LanePaint>,
    pub joins_in: Vec<GraphEdge>,
    pub edges_out: Vec<GraphEdge>,
    pub node_id: LaneId,
    pub node_col: usize,
    pub is_merge: bool,
}

#[derive(Clone, Debug)]
struct LaneState {
    id: LaneId,
    color: Rgba,
    target: CommitId,
}

pub fn compute_graph(commits: &[&Commit], theme: AppTheme) -> Vec<GraphRow> {
    let mut palette: Vec<Rgba> = Vec::new();
    for i in 0..24 {
        let hue = (i as f32 * 0.13) % 1.0;
        let sat = 0.75;
        let light = if theme.is_dark { 0.62 } else { 0.45 };
        palette.push(gpui::hsla(hue, sat, light, 1.0).into());
    }

    let known: HashSet<&str> = commits.iter().copied().map(|c| c.id.as_ref()).collect();
    let by_id: HashMap<&str, &Commit> = commits
        .iter()
        .copied()
        .map(|c| (c.id.as_ref(), c))
        .collect();

    // Approximate the "main line" as the first-parent chain from the first commit in the list,
    // which is typically the checked-out branch HEAD in our log view.
    let mut head_chain: HashSet<String> = HashSet::new();
    if let Some(&first) = commits.first() {
        let mut cur = first.id.clone();
        loop {
            if !head_chain.insert(cur.as_ref().to_string()) {
                break;
            }
            let Some(commit) = by_id.get(cur.as_ref()) else {
                break;
            };
            let Some(parent) = commit
                .parent_ids
                .first()
                .filter(|p| known.contains(p.as_ref()))
                .cloned()
            else {
                break;
            };
            cur = parent;
        }
    }

    let mut next_id: u64 = 1;
    let mut next_color: usize = 0;
    let mut lanes: Vec<LaneState> = Vec::new();
    let mut rows: Vec<GraphRow> = Vec::with_capacity(commits.len());
    let mut main_lane_id: Option<LaneId> = None;

    for commit in commits.iter().copied() {
        let incoming_ids = lanes.iter().map(|l| l.id).collect::<Vec<_>>();

        let mut hits = lanes
            .iter()
            .enumerate()
            .filter_map(|(ix, l)| (l.target == commit.id).then_some(ix))
            .collect::<Vec<_>>();

        if hits.is_empty() {
            let id = LaneId(next_id);
            next_id += 1;
            let color = palette[next_color % palette.len()];
            next_color += 1;
            lanes.push(LaneState {
                id,
                color,
                target: commit.id.clone(),
            });
            hits.push(lanes.len() - 1);
        }

        let is_merge = commit.parent_ids.len() > 1;
        let parent_ids = commit
            .parent_ids
            .iter()
            .filter(|p| known.contains(p.as_ref()))
            .cloned()
            .collect::<Vec<_>>();

        // Snapshot of lanes used for drawing this row (including any lanes that have converged
        // onto this commit before we re-target them to parents).
        let lanes_now = lanes
            .iter()
            .map(|l| LanePaint {
                id: l.id,
                color: l.color,
            })
            .collect::<Vec<_>>();

        let node_col = if let Some(main_lane_id) = main_lane_id
            && head_chain.contains(commit.id.as_ref())
        {
            hits.iter()
                .copied()
                .find(|ix| lanes.get(*ix).is_some_and(|lane| lane.id == main_lane_id))
                .unwrap_or_else(|| *hits.first().unwrap())
        } else {
            *hits.first().unwrap()
        };
        let node_id = lanes[node_col].id;
        if main_lane_id.is_none() {
            main_lane_id = Some(node_id);
        }

        // Incoming join edges: other lanes that were targeting this commit join into the node.
        let mut joins_in = Vec::new();
        for &col in hits.iter().skip(1) {
            joins_in.push(GraphEdge {
                from_col: col,
                to_col: node_col,
                color: lanes[col].color,
            });
        }

        // Assign parents to lanes that were already targeting this commit:
        // - Node lane follows first parent.
        // - Remaining hit lanes (if any) follow subsequent parents in order.
        // - Extra hit lanes beyond available parents are ended.
        if let Some(first_parent) = parent_ids.first().cloned() {
            lanes[node_col].target = first_parent;
        } else {
            // No parents: end lane.
            // We'll drop ended lanes after recording lanes_next.
            lanes[node_col].target = commit.id.clone();
        }

        let mut covered_parents = 1usize;
        for (hit_ix, parent) in hits.iter().skip(1).zip(parent_ids.iter().skip(1)) {
            lanes[*hit_ix].target = parent.clone();
            covered_parents += 1;
        }

        for &hit_ix in hits.iter().skip(1 + parent_ids.len().saturating_sub(1)) {
            // End lanes that converged here but don't have a parent to follow.
            lanes[hit_ix].target = commit.id.clone();
        }

        // Create lanes for any remaining parents not covered by existing converged lanes.
        if parent_ids.len() > covered_parents {
            let mut insert_at = node_col + 1;
            for parent in parent_ids.iter().skip(covered_parents) {
                // If another lane already targets this parent, reuse it.
                if lanes.iter().any(|l| l.target == *parent) {
                    continue;
                }
                let id = LaneId(next_id);
                next_id += 1;
                let color = palette[next_color % palette.len()];
                next_color += 1;
                lanes.insert(
                    insert_at,
                    LaneState {
                        id,
                        color,
                        target: parent.clone(),
                    },
                );
                insert_at += 1;
            }
        }

        // Remove ended lanes: lanes whose target is not part of the visible graph, or whose target
        // is this commit without a parent to follow.
        lanes.retain(|l| {
            known.contains(l.target.as_ref()) && l.target.as_ref() != commit.id.as_ref()
        });

        let lanes_next = lanes
            .iter()
            .map(|l| LanePaint {
                id: l.id,
                color: l.color,
            })
            .collect::<Vec<_>>();

        // Node->parent "merge" edges: connect the node into secondary-parent lanes.
        // - If the secondary parent lane existed already in this row, draw an explicit edge.
        // - If it was inserted this row, the continuation line already originates from the node.
        let mut edges_out = Vec::new();
        let mut next_index_by_lane: HashMap<LaneId, usize> = HashMap::new();
        for (ix, lane) in lanes_next.iter().enumerate() {
            next_index_by_lane.insert(lane.id, ix);
        }
        let lanes_now_ids: HashSet<LaneId> = lanes_now.iter().map(|l| l.id).collect();
        for parent in parent_ids.iter().skip(1) {
            if let Some(lane) = lanes
                .iter()
                .find(|l| l.target == *parent && lanes_now_ids.contains(&l.id))
                && let Some(to_col) = next_index_by_lane.get(&lane.id).copied()
            {
                edges_out.push(GraphEdge {
                    from_col: node_col,
                    to_col,
                    color: lanes_next[to_col].color,
                });
            }
        }

        rows.push(GraphRow {
            incoming_ids,
            lanes_now,
            lanes_next,
            joins_in,
            edges_out,
            node_id,
            node_col,
            is_merge,
        });
    }

    rows
}
