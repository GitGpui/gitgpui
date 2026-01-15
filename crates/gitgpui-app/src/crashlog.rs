use std::backtrace::Backtrace;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

static WRITING_CRASH_LOG: AtomicBool = AtomicBool::new(false);

pub fn install() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        write_panic_log(info);
        previous(info);
    }));
}

fn write_panic_log(info: &std::panic::PanicHookInfo<'_>) {
    if WRITING_CRASH_LOG
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    let _guard = ResetFlagOnDrop;

    let Some(dir) = crash_dir() else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);

    let Some(path) = crash_log_path(&dir) else {
        return;
    };

    let mut file = match open_append(&path) {
        Ok(f) => f,
        Err(_) => return,
    };

    let _ = writeln!(file, "=== GitGpui crash (panic) ===");
    let _ = writeln!(file, "timestamp_unix_ms={}", unix_time_ms());
    let _ = writeln!(file, "crate={} version={}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    let _ = writeln!(file, "thread={}", std::thread::current().name().unwrap_or("<unnamed>"));

    if let Some(location) = info.location() {
        let _ = writeln!(
            file,
            "location={}#L{}",
            location.file(),
            location.line()
        );
    }

    let payload = info
        .payload()
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| info.payload().downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "<non-string panic payload>".to_string());
    let _ = writeln!(file, "message={payload}");
    let _ = writeln!(file, "info={info}");

    let bt = Backtrace::force_capture();
    let _ = writeln!(file, "backtrace:\n{bt}");
    let _ = writeln!(file);
    let _ = file.flush();
}

fn crash_dir() -> Option<PathBuf> {
    let base = if cfg!(target_os = "linux") {
        if let Ok(state) = std::env::var("XDG_STATE_HOME")
            && !state.trim().is_empty()
        {
            PathBuf::from(state)
        } else if let Ok(home) = std::env::var("HOME")
            && !home.trim().is_empty()
        {
            PathBuf::from(home).join(".local").join("state")
        } else {
            return None;
        }
    } else if cfg!(target_os = "macos") {
        if let Ok(home) = std::env::var("HOME")
            && !home.trim().is_empty()
        {
            PathBuf::from(home).join("Library").join("Logs")
        } else {
            return None;
        }
    } else if cfg!(target_os = "windows") {
        if let Ok(local) = std::env::var("LOCALAPPDATA")
            && !local.trim().is_empty()
        {
            PathBuf::from(local)
        } else if let Ok(appdata) = std::env::var("APPDATA")
            && !appdata.trim().is_empty()
        {
            PathBuf::from(appdata)
        } else {
            return None;
        }
    } else {
        if let Ok(home) = std::env::var("HOME")
            && !home.trim().is_empty()
        {
            PathBuf::from(home)
        } else {
            return None;
        }
    };

    Some(base.join("gitgpui").join("crashes"))
}

fn crash_log_path(dir: &PathBuf) -> Option<PathBuf> {
    let pid = std::process::id();
    Some(dir.join(format!("panic-{pid}-{}.log", unix_time_ms())))
}

fn open_append(path: &PathBuf) -> std::io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

fn unix_time_ms() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

struct ResetFlagOnDrop;

impl Drop for ResetFlagOnDrop {
    fn drop(&mut self) {
        WRITING_CRASH_LOG.store(false, Ordering::SeqCst);
    }
}
