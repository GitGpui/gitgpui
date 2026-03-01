mod cli;
mod crashlog;

use cli::{AppMode, exit_code};

fn main() {
    let mode = match cli::parse_app_mode() {
        Ok(mode) => mode,
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(exit_code::ERROR);
        }
    };

    #[cfg(feature = "ui")]
    {
        crashlog::install();

        use gitgpui_core::services::GitBackend;
        use std::sync::Arc;

        let backend: Arc<dyn GitBackend> = if cfg!(feature = "gix") {
            #[cfg(feature = "gix")]
            {
                Arc::new(gitgpui_git_gix::GixBackend)
            }

            #[cfg(not(feature = "gix"))]
            {
                gitgpui_git::default_backend()
            }
        } else {
            gitgpui_git::default_backend()
        };

        match mode {
            AppMode::Browser { path } => {
                // Pass path to the UI layer. The existing run() reads
                // std::env::args_os().nth(1) internally, so for now we
                // ignore `path` here — it is parsed for future use.
                let _ = path;

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
            }
            AppMode::Difftool(config) => {
                // TODO: launch focused diff view via GPUI
                eprintln!(
                    "difftool mode: local={} remote={}",
                    config.local.display(),
                    config.remote.display()
                );
                if let Some(ref p) = config.display_path {
                    eprintln!("  display path: {p}");
                }
                eprintln!(
                    "Focused difftool UI is not yet implemented. \
                     Use the full browser for now."
                );
                std::process::exit(exit_code::ERROR);
            }
            AppMode::Mergetool(config) => {
                // TODO: launch focused merge view via GPUI
                eprintln!(
                    "mergetool mode: merged={} local={} remote={}",
                    config.merged.display(),
                    config.local.display(),
                    config.remote.display()
                );
                if let Some(ref b) = config.base {
                    eprintln!("  base: {}", b.display());
                }
                eprintln!(
                    "Focused mergetool UI is not yet implemented. \
                     Use the full browser for now."
                );
                std::process::exit(exit_code::ERROR);
            }
        }
    }

    #[cfg(not(feature = "ui"))]
    {
        let _ = mode;
        eprintln!("GitGpui UI is disabled. Build with `-p gitgpui-app --features ui`.");
    }
}
