use serde::{de::Deserializer, Deserialize};
use zbus::zvariant::Type;

use crate::{AppID, WindowIdentifierType};

pub type Result<T> = std::result::Result<T, crate::error::PortalError>;

#[derive(Debug, Default, Type)]
#[zvariant(signature = "s")]
pub(crate) struct MaybeWindowIdentifier(Option<WindowIdentifierType>);

impl MaybeWindowIdentifier {
    pub fn inner(self) -> Option<WindowIdentifierType> {
        self.0
    }
}

impl<'de> Deserialize<'de> for MaybeWindowIdentifier {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let inner = String::deserialize(deserializer)?;
        if inner.is_empty() {
            Ok(Self(None))
        } else {
            Ok(Self(inner.parse::<WindowIdentifierType>().ok()))
        }
    }
}

#[derive(Debug, Default, Type)]
#[zvariant(signature = "s")]
pub(crate) struct MaybeAppID(Option<AppID>);

impl MaybeAppID {
    pub fn inner(self) -> Option<AppID> {
        self.0
    }
}

impl<'de> Deserialize<'de> for MaybeAppID {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let inner = String::deserialize(deserializer)?;
        if inner.is_empty() {
            Ok(Self(None))
        } else {
            Ok(Self(inner.parse::<AppID>().ok()))
        }
    }
}

pub mod access;
pub mod account;
pub mod app_chooser;
pub mod background;
mod builder;
pub use builder::Builder;
pub mod email;
pub mod file_chooser;
pub mod lockdown;
pub mod permission_store;
pub mod print;
pub mod request;
pub mod screenshot;
pub mod secret;
pub mod settings;
pub mod wallpaper;
