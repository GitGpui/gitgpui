use super::*;

impl GitGpuiView {
    pub(in crate::view) fn prompt_open_repo(
        &mut self,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        let store = Arc::clone(&self.store);
        let view = cx.weak_entity();

        let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open Git Repository".into()),
        });

        window
            .spawn(cx, async move |cx| {
                let result = rx.await;
                let paths = match result {
                    Ok(Ok(Some(paths))) => paths,
                    Ok(Ok(None)) => return,
                    Ok(Err(_)) | Err(_) => {
                        let _ = view.update(cx, |this, cx| {
                            this.open_repo_panel = true;
                            cx.notify();
                        });
                        return;
                    }
                };

                let Some(path) = paths.into_iter().next() else {
                    return;
                };

                let is_repo_root = smol::unblock({
                    let path_for_task = path.clone();
                    move || {
                        let dot_git = path_for_task.join(".git");
                        if dot_git.is_dir() {
                            return true;
                        }
                        if !dot_git.is_file() {
                            return false;
                        }

                        let Ok(contents) = std::fs::read_to_string(&dot_git) else {
                            return false;
                        };
                        let Some(line) = contents.lines().next() else {
                            return false;
                        };
                        let line = line.trim();
                        let Some(gitdir) = line.strip_prefix("gitdir:") else {
                            return false;
                        };
                        let gitdir = gitdir.trim();
                        if gitdir.is_empty() {
                            return false;
                        }
                        let gitdir_path = std::path::Path::new(gitdir);
                        let resolved = if gitdir_path.is_absolute() {
                            gitdir_path.to_path_buf()
                        } else {
                            path_for_task.join(gitdir_path)
                        };
                        resolved.is_dir()
                    }
                })
                .await;

                if is_repo_root {
                    store.dispatch(Msg::OpenRepo(path));
                    let _ = view.update(cx, |this, cx| {
                        this.open_repo_panel = false;
                        cx.notify();
                    });
                } else {
                    let _ = view.update(cx, |this, cx| {
                        this.open_repo_panel = true;
                        this.open_repo_input.update(cx, |input, cx| {
                            input.set_text(path.display().to_string(), cx)
                        });
                        cx.notify();
                    });
                }
            })
            .detach();
    }
}
