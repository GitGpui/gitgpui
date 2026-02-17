use crate::model::{AppState, RepoId};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::{env, fs, io};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UiSession {
    pub open_repos: Vec<PathBuf>,
    pub active_repo: Option<PathBuf>,
    pub window_width: Option<u32>,
    pub window_height: Option<u32>,
    pub sidebar_width: Option<u32>,
    pub details_width: Option<u32>,
    pub date_time_format: Option<String>,
    pub history_show_author: Option<bool>,
    pub history_show_date: Option<bool>,
    pub history_show_sha: Option<bool>,
    pub terminal_program: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct UiSessionFileV1 {
    version: u32,
    open_repos: Vec<String>,
    active_repo: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct UiSessionFileV2 {
    version: u32,
    open_repos: Vec<String>,
    active_repo: Option<String>,
    window_width: Option<u32>,
    window_height: Option<u32>,
    sidebar_width: Option<u32>,
    details_width: Option<u32>,
    date_time_format: Option<String>,
    history_show_author: Option<bool>,
    history_show_date: Option<bool>,
    history_show_sha: Option<bool>,
    terminal_program: Option<String>,
}

const SESSION_FILE_VERSION_V1: u32 = 1;
const SESSION_FILE_VERSION_V2: u32 = 2;
const CURRENT_SESSION_FILE_VERSION: u32 = SESSION_FILE_VERSION_V2;

pub fn load() -> UiSession {
    let Some(path) = default_session_file_path() else {
        return UiSession::default();
    };

    load_from_path(&path)
}

pub fn load_from_path(path: &Path) -> UiSession {
    let Some(file) = load_file_v2(path) else {
        return UiSession::default();
    };

    let (open_repos, active_repo) = parse_repos(file.open_repos, file.active_repo);
    UiSession {
        open_repos,
        active_repo,
        window_width: file.window_width,
        window_height: file.window_height,
        sidebar_width: file.sidebar_width,
        details_width: file.details_width,
        date_time_format: file.date_time_format,
        history_show_author: file.history_show_author,
        history_show_date: file.history_show_date,
        history_show_sha: file.history_show_sha,
        terminal_program: file.terminal_program,
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

    let mut file = load_file_v2(path).unwrap_or_default();
    file.version = CURRENT_SESSION_FILE_VERSION;
    file.open_repos = open_repos;
    file.active_repo = active_repo;

    persist_to_path(path, &file)
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UiSettings {
    pub window_width: Option<u32>,
    pub window_height: Option<u32>,
    pub sidebar_width: Option<u32>,
    pub details_width: Option<u32>,
    pub date_time_format: Option<String>,
    pub history_show_author: Option<bool>,
    pub history_show_date: Option<bool>,
    pub history_show_sha: Option<bool>,
    pub terminal_program: Option<String>,
}

pub fn persist_ui_settings(settings: UiSettings) -> io::Result<()> {
    let Some(path) = default_session_file_path() else {
        return Ok(());
    };
    persist_ui_settings_to_path(settings, &path)
}

pub fn persist_ui_settings_to_path(settings: UiSettings, path: &Path) -> io::Result<()> {
    let mut file = load_file_v2(path).unwrap_or_default();
    file.version = CURRENT_SESSION_FILE_VERSION;
    if settings.window_width.is_some() && settings.window_height.is_some() {
        file.window_width = settings.window_width;
        file.window_height = settings.window_height;
    }
    if let Some(w) = settings.sidebar_width {
        file.sidebar_width = Some(w);
    }
    if let Some(w) = settings.details_width {
        file.details_width = Some(w);
    }
    if let Some(fmt) = settings.date_time_format {
        file.date_time_format = Some(fmt);
    }
    if let Some(value) = settings.history_show_author {
        file.history_show_author = Some(value);
    }
    if let Some(value) = settings.history_show_date {
        file.history_show_date = Some(value);
    }
    if let Some(value) = settings.history_show_sha {
        file.history_show_sha = Some(value);
    }
    file.terminal_program = settings.terminal_program;

    persist_to_path(path, &file)
}

fn parse_repos(
    open_repos_raw: Vec<String>,
    active_repo_raw: Option<String>,
) -> (Vec<PathBuf>, Option<PathBuf>) {
    let mut open_repos: Vec<PathBuf> = Vec::new();
    for repo in open_repos_raw {
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

    let active_repo = active_repo_raw
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

    (open_repos, active_repo)
}

fn load_file_v2(path: &Path) -> Option<UiSessionFileV2> {
    let Ok(contents) = fs::read_to_string(path) else {
        return None;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return None;
    };
    let version = value
        .get("version")
        .and_then(|v| v.as_u64())
        .unwrap_or(SESSION_FILE_VERSION_V1 as u64) as u32;
    match version {
        SESSION_FILE_VERSION_V1 => {
            let file: UiSessionFileV1 = serde_json::from_value(value).ok()?;
            Some(UiSessionFileV2 {
                version: CURRENT_SESSION_FILE_VERSION,
                open_repos: file.open_repos,
                active_repo: file.active_repo,
                ..UiSessionFileV2::default()
            })
        }
        SESSION_FILE_VERSION_V2 => serde_json::from_value::<UiSessionFileV2>(value).ok(),
        _ => None,
    }
}

fn active_repo_path(state: &AppState, active_repo_id: Option<RepoId>) -> Option<&Path> {
    let active_repo_id = active_repo_id?;
    state
        .repos
        .iter()
        .find(|r| r.id == active_repo_id)
        .map(|r| r.spec.workdir.as_path())
}

fn persist_to_path(path: &Path, session: &impl Serialize) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let tmp_path = path.with_extension("json.tmp");
    let contents = serde_json::to_vec(session).expect("serializing session file should succeed");
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
        Some(PathBuf::from(home).join(".local/state/gitgpui"))
    }

    #[cfg(target_os = "macos")]
    {
        let home = env::var_os("HOME")?;
        return Some(PathBuf::from(home).join("Library/Application Support/gitgpui"));
    }

    #[cfg(target_os = "windows")]
    {
        let appdata = env::var_os("LOCALAPPDATA").or_else(|| env::var_os("APPDATA"))?;
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
        let dir = env::temp_dir().join(format!("gitgpui-session-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("session.json");

        let file = UiSessionFileV1 {
            version: SESSION_FILE_VERSION_V1,
            open_repos: vec!["/a".into(), "/b".into()],
            active_repo: Some("/b".into()),
        };
        persist_to_path(&path, &file).expect("persist succeeds");

        let contents = fs::read_to_string(&path).expect("read succeeds");
        let loaded: UiSessionFileV1 = serde_json::from_str(&contents).expect("json parses");
        assert_eq!(loaded.version, SESSION_FILE_VERSION_V1);
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
            RepoState::new_opening(
                RepoId(1),
                RepoSpec {
                    workdir: repo_a.clone(),
                },
            ),
            RepoState::new_opening(
                RepoId(2),
                RepoSpec {
                    workdir: repo_b.clone(),
                },
            ),
        ];
        state.active_repo = Some(RepoId(2));

        persist_from_state_to_path(&state, &path).expect("persist succeeds");
        let loaded = load_from_path(&path);
        assert_eq!(loaded.open_repos, vec![repo_a, repo_b.clone()]);
        assert_eq!(loaded.active_repo, Some(repo_b));
    }

    #[test]
    fn load_from_path_migrates_v1_files() {
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

        persist_to_path(
            &path,
            &UiSessionFileV1 {
                version: SESSION_FILE_VERSION_V1,
                open_repos: vec![
                    repo_a.to_string_lossy().to_string(),
                    repo_b.to_string_lossy().to_string(),
                ],
                active_repo: Some(repo_b.to_string_lossy().to_string()),
            },
        )
        .expect("persist succeeds");

        let loaded = load_from_path(&path);
        assert_eq!(loaded.open_repos, vec![repo_a, repo_b.clone()]);
        assert_eq!(loaded.active_repo, Some(repo_b));
        assert_eq!(loaded.window_width, None);
        assert_eq!(loaded.date_time_format, None);
    }

    #[test]
    fn persist_ui_settings_round_trips_date_time_format() {
        let dir = env::temp_dir().join(format!(
            "gitgpui-ui-settings-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("session.json");

        persist_to_path(
            &path,
            &UiSessionFileV2 {
                version: CURRENT_SESSION_FILE_VERSION,
                open_repos: Vec::new(),
                active_repo: None,
                ..UiSessionFileV2::default()
            },
        )
        .expect("seed session file");

        persist_ui_settings_to_path(
            UiSettings {
                window_width: None,
                window_height: None,
                sidebar_width: None,
                details_width: None,
                date_time_format: Some("ymd_hm_utc".to_string()),
                history_show_author: None,
                history_show_date: None,
                history_show_sha: None,
                terminal_program: None,
            },
            &path,
        )
        .expect("persist ui settings");

        let loaded = load_from_path(&path);
        assert_eq!(loaded.date_time_format.as_deref(), Some("ymd_hm_utc"));
    }
}
