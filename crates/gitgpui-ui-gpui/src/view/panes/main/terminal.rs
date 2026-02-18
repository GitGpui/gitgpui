use super::*;

impl MainPaneView {
    pub(in super::super::super) fn set_terminal_program(
        &mut self,
        next: Option<String>,
        cx: &mut gpui::Context<Self>,
    ) {
        self.terminal_program = next.and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });
        cx.notify();
    }

    pub(in super::super::super) fn toggle_terminal_panel(
        &mut self,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        self.terminal_open = !self.terminal_open;
        if self.terminal_open {
            let theme = self.theme;
            self.terminal_output_input.update(cx, |input, cx| {
                input.set_theme(theme, cx);
                input.set_text(self.terminal_buffer.clone(), cx);
            });
            self.terminal_command_input.update(cx, |input, cx| {
                input.set_theme(theme, cx);
            });

            let focus = self
                .terminal_command_input
                .read_with(cx, |input, _| input.focus_handle());
            window.focus(&focus);
        }
        self.notify_fingerprint = Self::notify_fingerprint_for(&self.state, self.terminal_open);
        cx.notify();
    }

    pub(in super::super::super) fn close_terminal_panel(&mut self, cx: &mut gpui::Context<Self>) {
        if !self.terminal_open {
            return;
        }
        self.terminal_open = false;
        self.notify_fingerprint = Self::notify_fingerprint_for(&self.state, self.terminal_open);
        cx.notify();
    }

    pub(in super::super::super) fn terminal_submit_command(
        &mut self,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.terminal_running {
            return;
        }
        let Some(workdir) = self.active_repo().map(|repo| repo.spec.workdir.clone()) else {
            self.terminal_append_output("No repository.\n", cx);
            return;
        };

        let command = self
            .terminal_command_input
            .read_with(cx, |input, _| input.text().trim().to_string());
        if command.is_empty() {
            return;
        }

        self.terminal_command_input.update(cx, |input, cx| {
            input.set_text("", cx);
        });

        self.terminal_append_output(&format!("$ {command}\n"), cx);
        self.terminal_running = true;
        cx.notify();

        let terminal_program = self.terminal_program.clone();
        cx.spawn(
            async move |pane: WeakEntity<MainPaneView>, cx: &mut gpui::AsyncApp| {
                let result = smol::unblock(move || {
                    terminal_run_shell_command(&workdir, terminal_program.as_deref(), &command)
                })
                .await;

                let _ = pane.update(cx, |this, cx| {
                    this.terminal_running = false;
                    match result {
                        Ok(output) => this.terminal_append_output(&output, cx),
                        Err(err) => this.terminal_append_output(&format!("{err}\n"), cx),
                    }
                    cx.notify();
                });
            },
        )
        .detach();
    }

    fn terminal_append_output(&mut self, text: &str, cx: &mut gpui::Context<Self>) {
        const MAX_CHARS: usize = 200_000;

        self.terminal_buffer.push_str(text);
        if self.terminal_buffer.len() > MAX_CHARS {
            let mut keep_from = self.terminal_buffer.len().saturating_sub(MAX_CHARS);
            if !self.terminal_buffer.is_char_boundary(keep_from) {
                keep_from = self
                    .terminal_buffer
                    .char_indices()
                    .find_map(|(ix, _)| (ix >= keep_from).then_some(ix))
                    .unwrap_or(0);
            }
            self.terminal_buffer = self.terminal_buffer[keep_from..].to_string();
        }

        let theme = self.theme;
        let next = self.terminal_buffer.clone();
        self.terminal_output_input.update(cx, |input, cx| {
            input.set_theme(theme, cx);
            input.set_text(next, cx);
        });
    }
}

fn terminal_run_shell_command(
    workdir: &std::path::Path,
    terminal_program: Option<&str>,
    command: &str,
) -> Result<String, String> {
    use std::process::Command;

    #[cfg(target_os = "windows")]
    {
        let program = terminal_program
            .map(ToOwned::to_owned)
            .or_else(|| std::env::var("COMSPEC").ok())
            .unwrap_or_else(|| "cmd".to_string());
        let program_lower = program.to_ascii_lowercase();

        let mut cmd = Command::new(&program);
        cmd.current_dir(workdir);

        if program_lower.ends_with("pwsh") || program_lower.ends_with("pwsh.exe") {
            cmd.args(["-NoLogo", "-NoProfile", "-Command", command]);
        } else if program_lower.ends_with("powershell") || program_lower.ends_with("powershell.exe")
        {
            cmd.args(["-NoLogo", "-NoProfile", "-Command", command]);
        } else {
            cmd.args(["/C", command]);
        }

        let output = cmd
            .output()
            .map_err(|err| format!("Failed to run terminal command: {err}"))?;

        let mut text = String::new();
        text.push_str(String::from_utf8_lossy(&output.stdout).as_ref());
        if !output.stderr.is_empty() {
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(String::from_utf8_lossy(&output.stderr).as_ref());
        }

        if !output.status.success() {
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(&format!("[exit status: {}]\n", output.status));
        } else if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }

        return Ok(text);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let program = terminal_program
            .map(ToOwned::to_owned)
            .or_else(|| std::env::var("SHELL").ok())
            .unwrap_or_else(|| "sh".to_string());

        let output = Command::new(&program)
            .current_dir(workdir)
            .args(["-lc", command])
            .output()
            .map_err(|err| format!("Failed to run terminal command: {err}"))?;

        let mut text = String::new();
        text.push_str(String::from_utf8_lossy(&output.stdout).as_ref());
        if !output.stderr.is_empty() {
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(String::from_utf8_lossy(&output.stderr).as_ref());
        }

        if !output.status.success() {
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(&format!("[exit status: {}]\n", output.status));
        } else if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }

        Ok(text)
    }
}
