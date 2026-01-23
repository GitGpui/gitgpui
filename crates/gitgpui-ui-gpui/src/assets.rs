use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;

pub struct GitGpuiAssets;

impl GitGpuiAssets {
    fn load_static(path: &str) -> Option<Cow<'static, [u8]>> {
        match path {
            "gitgpui_logo.svg" => Some(Cow::Borrowed(include_bytes!(
                "../../../assets/gitgpui_logo.svg"
            ))),
            "gitgpui_logo_window.svg" => Some(Cow::Borrowed(include_bytes!(
                "../../../assets/gitgpui_logo_window.svg"
            ))),
            "icons/arrow_down.svg" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/arrow_down.svg"
            ))),
            "icons/arrow_up.svg" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/arrow_up.svg"
            ))),
            "icons/box.svg" => Some(Cow::Borrowed(include_bytes!("../assets/icons/box.svg"))),
            "icons/chevron_down.svg" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/chevron_down.svg"
            ))),
            "icons/cloud.svg" => Some(Cow::Borrowed(include_bytes!("../assets/icons/cloud.svg"))),
            "icons/computer.svg" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/computer.svg"
            ))),
            "icons/folder.svg" => Some(Cow::Borrowed(include_bytes!("../assets/icons/folder.svg"))),
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
            "icons/gitgpui_mark.svg" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/gitgpui_mark.svg"
            ))),
            "icons/menu.svg" => Some(Cow::Borrowed(include_bytes!("../assets/icons/menu.svg"))),
            _ => None,
        }
    }

    fn list_static(dir: &str) -> Vec<SharedString> {
        match dir.trim_end_matches('/') {
            "" => vec![
                "gitgpui_logo.svg".into(),
                "gitgpui_logo_window.svg".into(),
                "icons".into(),
            ],
            "icons" => vec![
                "icons/arrow_down.svg".into(),
                "icons/arrow_up.svg".into(),
                "icons/box.svg".into(),
                "icons/chevron_down.svg".into(),
                "icons/cloud.svg".into(),
                "icons/computer.svg".into(),
                "icons/folder.svg".into(),
                "icons/generic_minimize.svg".into(),
                "icons/generic_maximize.svg".into(),
                "icons/generic_restore.svg".into(),
                "icons/generic_close.svg".into(),
                "icons/git_branch.svg".into(),
                "icons/gitgpui_mark.svg".into(),
                "icons/menu.svg".into(),
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
