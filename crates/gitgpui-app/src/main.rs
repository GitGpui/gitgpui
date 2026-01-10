fn main() {
    #[cfg(feature = "ui")]
    {
        use gitgpui_core::services::GitBackend;
        use std::sync::Arc;

        let backend: Arc<dyn GitBackend> = if cfg!(feature = "gix") {
            #[cfg(feature = "gix")]
            {
                Arc::new(gitgpui_git_gix::GixBackend::default())
            }

            #[cfg(not(feature = "gix"))]
            {
                gitgpui_git::default_backend()
            }
        } else {
            gitgpui_git::default_backend()
        };

        if cfg!(feature = "ui-gpui") {
            #[cfg(feature = "ui-gpui")]
            {
                gitgpui_ui_gpui::run(backend);
            }

            #[cfg(not(feature = "ui-gpui"))]
            {
                gitgpui_ui::run(backend);
            }
        } else {
            gitgpui_ui::run(backend);
        }
        return;
    }

    #[cfg(not(feature = "ui"))]
    {
        eprintln!("GitGpui UI is disabled. Build with `-p gitgpui-app --features ui`.");
    }
}
