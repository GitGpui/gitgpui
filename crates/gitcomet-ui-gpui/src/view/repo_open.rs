use super::*;

impl GitCometView {
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

                // Let the backend decide whether the path is a repository.
                // Frontend checks are brittle across bare repos/worktrees/submodules.
                store.dispatch(Msg::OpenRepo(path));
                let _ = view.update(cx, |this, cx| {
                    this.open_repo_panel = false;
                    cx.notify();
                });
            })
            .detach();
    }
}
