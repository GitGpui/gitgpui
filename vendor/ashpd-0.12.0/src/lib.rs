#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(missing_docs)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/bilelmoussaoui/ashpd/master/ashpd-demo/data/icons/com.belmoussaoui.ashpd.demo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/bilelmoussaoui/ashpd/master/ashpd-demo/data/icons/com.belmoussaoui.ashpd.demo-symbolic.svg"
)]
#![doc = include_str!("../README.md")]
#[cfg(all(all(feature = "tokio", feature = "async-std"), not(doc)))]
compile_error!("You can't enable both async-std & tokio features at once");
#[cfg(all(not(feature = "tokio"), not(feature = "async-std"), not(doc)))]
compile_error!("Either the `async-std` or the `tokio` feature has to be enabled");

/// Alias for a [`Result`] with the error type `ashpd::Error`.
pub type Result<T> = std::result::Result<T, Error>;

static IS_SANDBOXED: OnceLock<bool> = OnceLock::new();

mod activation_token;
/// Interact with the user's desktop such as taking a screenshot, setting a
/// background or querying the user's location.
pub mod desktop;
/// Interact with the documents store or transfer files across apps.
pub mod documents;
mod error;
mod window_identifier;

pub use self::{activation_token::ActivationToken, window_identifier::WindowIdentifier};
mod app_id;
mod registry;
pub use self::{app_id::AppID, registry::register_host_app};
mod file_path;
pub use self::file_path::FilePath;

mod proxy;

#[cfg(feature = "backend")]
#[cfg_attr(docsrs, doc(cfg(feature = "backend")))]
pub use self::window_identifier::WindowIdentifierType;
#[cfg(feature = "backend")]
#[cfg_attr(docsrs, doc(cfg(feature = "backend")))]
#[allow(missing_docs)]
/// Build your custom portals backend.
pub mod backend;
/// Spawn commands outside the sandbox or monitor if the running application has
/// received an update & install it.
pub mod flatpak;
mod helpers;
use std::sync::OnceLock;

#[cfg(feature = "backend")]
#[cfg_attr(docsrs, doc(cfg(feature = "backend")))]
pub use async_trait;
pub use enumflags2;
pub use url;
pub use zbus::{self, zvariant};

/// Check whether the application is running inside a sandbox.
///
/// The function checks whether the file `/.flatpak-info` exists, or if the app
/// is running as a snap, or if the environment variable `GTK_USE_PORTAL` is set
/// to `1`. As the return value of this function will not change during the
/// runtime of a program; it is cached for future calls.
pub async fn is_sandboxed() -> bool {
    if let Some(cached_value) = IS_SANDBOXED.get() {
        return *cached_value;
    }
    let new_value = crate::helpers::is_flatpak().await
        || crate::helpers::is_snap().await
        || std::env::var("GTK_USE_PORTAL")
            .map(|v| v == "1")
            .unwrap_or(false);

    *IS_SANDBOXED.get_or_init(|| new_value)
}

pub use self::error::{Error, PortalError};

mod sealed {
    /// Use as a supertrait for public traits that users should not be able to
    /// implement
    pub trait Sealed {}
}

pub(crate) use sealed::Sealed;

/// Process ID.
///
/// Matches the type used in std.
pub type Pid = u32;
