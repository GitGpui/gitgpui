use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;

pub struct GitGpuiAssets;

impl GitGpuiAssets {
    fn load_static(path: &str) -> Option<Cow<'static, [u8]>> {
        match path {
            "icons/arrow_down.svg" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/arrow_down.svg"
            ))),
            "icons/arrow_up.svg" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/arrow_up.svg"
            ))),
            "icons/box.svg" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/box.svg"
            ))),
            "icons/chevron_down.svg" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/chevron_down.svg"
            ))),
            "icons/generic_minimize.svg" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/generic_minimize.svg"
            ))),
            "icons/generic_maximize.svg" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/generic_maximize.svg"
            ))),
            "icons/generic_restore.svg" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/generic_restore.svg"
            ))),
            "icons/generic_close.svg" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/generic_close.svg"
            ))),
            "icons/git_branch.svg" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/git_branch.svg"
            ))),
            _ => None,
        }
    }

    fn list_static(dir: &str) -> Vec<SharedString> {
        match dir.trim_end_matches('/') {
            "" => vec!["icons".into()],
            "icons" => vec![
                "icons/arrow_down.svg".into(),
                "icons/arrow_up.svg".into(),
                "icons/box.svg".into(),
                "icons/chevron_down.svg".into(),
                "icons/generic_minimize.svg".into(),
                "icons/generic_maximize.svg".into(),
                "icons/generic_restore.svg".into(),
                "icons/generic_close.svg".into(),
                "icons/git_branch.svg".into(),
            ],
            _ => vec![],
        }
    }
}

impl AssetSource for GitGpuiAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(Self::load_static(path))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Self::list_static(path))
    }
}
