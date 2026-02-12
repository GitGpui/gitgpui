use super::GixRepo;
use crate::util::run_git_capture;
use gitgpui_core::domain::{Branch, CommitId, Upstream, UpstreamDivergence};
use gitgpui_core::services::Result;
use std::process::Command;
use std::str;

impl GixRepo {
    pub(super) fn current_branch_impl(&self) -> Result<String> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("rev-parse")
            .arg("--abbrev-ref")
            .arg("HEAD");
        Ok(run_git_capture(cmd, "git rev-parse --abbrev-ref HEAD")?
            .trim()
            .to_string())
    }

    pub(super) fn list_branches_impl(&self) -> Result<Vec<Branch>> {
        fn parse_upstream_short(s: &str) -> Option<Upstream> {
            let s = s.trim();
            if s.is_empty() {
                return None;
            }
            let (remote, branch) = s.split_once('/')?;
            Some(Upstream {
                remote: remote.to_string(),
                branch: branch.to_string(),
            })
        }

        fn parse_upstream_track(s: &str) -> Option<UpstreamDivergence> {
            let s = s.trim();
            if s.is_empty() {
                return None;
            }
            let s = s.trim_start_matches('[').trim_end_matches(']');
            if s.trim().is_empty() || s.contains("gone") {
                return None;
            }

            let mut ahead: Option<usize> = None;
            let mut behind: Option<usize> = None;

            for part in s.split(',') {
                let mut it = part.trim().split_whitespace();
                let Some(kind) = it.next() else {
                    continue;
                };
                let Some(n) = it.next().and_then(|x| x.parse::<usize>().ok()) else {
                    continue;
                };
                match kind {
                    "ahead" => ahead = Some(n),
                    "behind" => behind = Some(n),
                    _ => {}
                }
            }

            let ahead = ahead.unwrap_or(0);
            let behind = behind.unwrap_or(0);
            Some(UpstreamDivergence { ahead, behind })
        }

        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("for-each-ref")
            .arg("--format=%(refname:short)\t%(objectname)\t%(upstream:short)\t%(upstream:track)")
            .arg("refs/heads");
        let stdout = run_git_capture(cmd, "git for-each-ref refs/heads")?;

        let mut branches = Vec::new();
        for line in stdout.lines() {
            let mut parts = line.split('\t');
            let Some(name) = parts.next().map(str::trim).filter(|s| !s.is_empty()) else {
                continue;
            };
            let Some(sha) = parts.next().map(str::trim).filter(|s| !s.is_empty()) else {
                continue;
            };
            let upstream_short = parts.next().unwrap_or("").trim();
            let track = parts.next().unwrap_or("").trim();

            let upstream = parse_upstream_short(upstream_short);
            let divergence = upstream.as_ref().and_then(|_| parse_upstream_track(track));

            branches.push(Branch {
                name: name.to_string(),
                target: CommitId(sha.to_string()),
                upstream,
                divergence,
            });
        }

        branches.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(branches)
    }
}
