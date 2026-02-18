use gpui::SharedString;
use rustc_hash::FxHashMap as HashMap;
use std::path::PathBuf;

pub(super) fn cached_path_display(
    cache: &mut HashMap<PathBuf, SharedString>,
    path: &PathBuf,
) -> SharedString {
    const MAX_ENTRIES: usize = 8_192;
    if cache.len() > MAX_ENTRIES {
        cache.clear();
    }
    if let Some(s) = cache.get(path) {
        return s.clone();
    }
    let s: SharedString = path.display().to_string().into();
    cache.insert(path.clone(), s.clone());
    s
}
