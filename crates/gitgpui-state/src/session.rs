use crate::model::{AppState, RepoId};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::{env, fs, io};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UiSession {
    pub open_repos: Vec<PathBuf>,
    pub active_repo: Option<PathBuf>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct UiSessionFileV1 {
    version: u32,
    open_repos: Vec<String>,
    active_repo: Option<String>,
}

const SESSION_FILE_VERSION: u32 = 1;

pub fn load() -> UiSession {
    let Some(path) = default_session_file_path() else {
        return UiSession::default();
    };

    load_from_path(&path)
}

pub fn load_from_path(path: &Path) -> UiSession {
    let Ok(contents) = fs::read_to_string(path) else {
        return UiSession::default();
    };

    let Ok(file) = serde_json::from_str::<UiSessionFileV1>(&contents) else {
        return UiSession::default();
    };

    if file.version != SESSION_FILE_VERSION {
        return UiSession::default();
    }

    let mut open_repos: Vec<PathBuf> = Vec::new();
    for repo in file.open_repos {
        let repo = repo.trim();
        if repo.is_empty() {
            continue;
        }
        let repo = PathBuf::from(repo);
        if open_repos.iter().any(|p| p == &repo) {
            continue;
        }
        open_repos.push(repo);
    }

    let active_repo = file
        .active_repo
        .as_deref()
        .and_then(|p| {
            let p = p.trim();
            if p.is_empty() {
                None
            } else {
                Some(PathBuf::from(p))
            }
        })
        .filter(|active| open_repos.iter().any(|p| p == active));

    UiSession {
        open_repos,
        active_repo,
    }
}

pub fn persist_from_state(state: &AppState) -> io::Result<()> {
    let Some(path) = default_session_file_path() else {
        return Ok(());
    };

    persist_from_state_to_path(state, &path)
}

pub fn persist_from_state_to_path(state: &AppState, path: &Path) -> io::Result<()> {
    let mut open_repos: Vec<String> = Vec::new();
    for repo in &state.repos {
        let s = repo.spec.workdir.to_string_lossy().to_string();
        if open_repos.iter().any(|p| p == &s) {
            continue;
        }
        open_repos.push(s);
    }

    let active_repo: Option<String> = active_repo_path(state, state.active_repo)
        .map(|p| p.to_string_lossy().to_string())
        .filter(|active| open_repos.iter().any(|p| p == active));

    let file = UiSessionFileV1 {
        version: SESSION_FILE_VERSION,
        open_repos,
        active_repo,
    };

    persist_to_path(path, &file)
}

fn active_repo_path<'a>(state: &'a AppState, active_repo_id: Option<RepoId>) -> Option<&'a Path> {
    let active_repo_id = active_repo_id?;
    state
        .repos
        .iter()
        .find(|r| r.id == active_repo_id)
        .map(|r| r.spec.workdir.as_path())
}

fn persist_to_path(path: &Path, session: &UiSessionFileV1) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let tmp_path = path.with_extension("json.tmp");
    let contents = serde_json::to_vec(session).expect("serializing UiSessionFileV1 should succeed");
    fs::write(&tmp_path, contents)?;

    match fs::rename(&tmp_path, path) {
        Ok(()) => Ok(()),
        Err(rename_err) => {
            // Windows can't overwrite an existing file via rename.
            let copy_res = fs::copy(&tmp_path, path);
            let _ = fs::remove_file(&tmp_path);
            match copy_res {
                Ok(_) => Ok(()),
                Err(copy_err) => Err(io::Error::new(
                    copy_err.kind(),
                    format!("rename failed: {rename_err}; copy failed: {copy_err}"),
                )),
            }
        }
    }
}

fn default_session_file_path() -> Option<PathBuf> {
    // Avoid writing to user state dir during unit tests unless explicitly exercised.
    if cfg!(test) {
        return None;
    }

    Some(app_state_dir()?.join("session.json"))
}

fn app_state_dir() -> Option<PathBuf> {
    // Follow XDG on linux; otherwise fall back to platform conventions.
    #[cfg(target_os = "linux")]
    {
        if let Some(state_home) = env::var_os("XDG_STATE_HOME") {
            return Some(PathBuf::from(state_home).join("gitgpui"));
        }
        let home = env::var_os("HOME")?;
        return Some(PathBuf::from(home).join(".local/state/gitgpui"));
    }

    #[cfg(target_os = "macos")]
    {
        let home = env::var_os("HOME")?;
        return Some(
            PathBuf::from(home).join("Library/Application Support/gitgpui"),
        );
    }

    #[cfg(target_os = "windows")]
    {
        let appdata = env::var_os("LOCALAPPDATA")
            .or_else(|| env::var_os("APPDATA"))?;
        return Some(PathBuf::from(appdata).join("gitgpui"));
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        env::var_os("HOME").map(|home| PathBuf::from(home).join(".gitgpui"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::RepoState;
    use gitgpui_core::domain::RepoSpec;

    #[test]
    fn session_file_round_trips() {
        let dir = env::temp_dir().join(format!(
            "gitgpui-session-test-{}",
            std::process::id()
        ));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("session.json");

        let file = UiSessionFileV1 {
            version: SESSION_FILE_VERSION,
            open_repos: vec!["/a".into(), "/b".into()],
            active_repo: Some("/b".into()),
        };
        persist_to_path(&path, &file).expect("persist succeeds");

        let contents = fs::read_to_string(&path).expect("read succeeds");
        let loaded: UiSessionFileV1 = serde_json::from_str(&contents).expect("json parses");
        assert_eq!(loaded.version, SESSION_FILE_VERSION);
        assert_eq!(loaded.open_repos, vec!["/a".to_string(), "/b".to_string()]);
        assert_eq!(loaded.active_repo.as_deref(), Some("/b"));
    }

    #[test]
    fn persist_from_state_and_load_from_path_round_trip() {
        let dir = env::temp_dir().join(format!(
            "gitgpui-session-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("session.json");

        let repo_a = dir.join("repo-a");
        let repo_b = dir.join("repo-b");
        let _ = fs::create_dir_all(&repo_a);
        let _ = fs::create_dir_all(&repo_b);

        let mut state = AppState::default();
        state.repos = vec![
            RepoState::new_opening(RepoId(1), RepoSpec { workdir: repo_a.clone() }),
            RepoState::new_opening(RepoId(2), RepoSpec { workdir: repo_b.clone() }),
        ];
        state.active_repo = Some(RepoId(2));

        persist_from_state_to_path(&state, &path).expect("persist succeeds");
        let loaded = load_from_path(&path);
        assert_eq!(loaded.open_repos, vec![repo_a, repo_b.clone()]);
        assert_eq!(loaded.active_repo, Some(repo_b));
    }
}
