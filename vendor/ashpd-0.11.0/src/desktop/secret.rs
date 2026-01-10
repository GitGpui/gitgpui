//! # Examples
//!
//! ```rust,no_run
//! use std::{io::Read, os::fd::AsFd};
//!
//! use ashpd::desktop::secret::Secret;
//!
//! async fn run() -> ashpd::Result<()> {
//!     let secret = Secret::new().await?;
//!
//!     let (mut x1, x2) = std::os::unix::net::UnixStream::pair()?;
//!     secret.retrieve(&x2).await?;
//!     drop(x2);
//!     let mut buf = Vec::new();
//!     x1.read_to_end(&mut buf)?;
//!
//!     Ok(())
//! }
//! ```

use std::os::fd::AsFd;

#[cfg(feature = "async-std")]
use async_net::{unix::UnixStream, Shutdown};
#[cfg(feature = "async-std")]
use futures_util::AsyncReadExt;
#[cfg(feature = "tokio")]
use tokio::{io::AsyncReadExt, io::AsyncWriteExt, net::UnixStream};
use zbus::zvariant::{Fd, SerializeDict, Type};

use super::{HandleToken, Request};
use crate::{proxy::Proxy, Error};

#[derive(SerializeDict, Type, Debug, Default)]
/// Specified options for a [`Secret::retrieve`] request.
#[zvariant(signature = "dict")]
struct RetrieveOptions {
    handle_token: HandleToken,
    /// A string returned by a previous call to `retrieve`.
    /// TODO: seems to not be used by the portal...
    token: Option<String>,
}

/// The interface lets sandboxed applications retrieve a per-application secret.
///
/// The secret can then be used for encrypting confidential data inside the
/// sandbox.
///
/// Wrapper of the DBus interface: [`org.freedesktop.portal.Secret`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Secret.html).
#[derive(Debug)]
#[doc(alias = "org.freedesktop.portal.Secret")]
pub struct Secret<'a>(Proxy<'a>);

impl<'a> Secret<'a> {
    /// Create a new instance of [`Secret`].
    pub async fn new() -> Result<Secret<'a>, Error> {
        let proxy = Proxy::new_desktop("org.freedesktop.portal.Secret").await?;
        Ok(Self(proxy))
    }

    /// Retrieves a master secret for a sandboxed application.
    ///
    /// # Arguments
    ///
    /// * `fd` - Writaeble file descriptor for transporting the secret.
    ///
    /// # Specifications
    ///
    /// See also [`RetrieveSecret`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Secret.html#org-freedesktop-portal-secret-retrievesecret)
    #[doc(alias = "RetrieveSecret")]
    pub async fn retrieve(&self, fd: &impl AsFd) -> Result<Request<()>, Error> {
        let options = RetrieveOptions::default();
        self.0
            .empty_request(
                &options.handle_token,
                "RetrieveSecret",
                &(Fd::from(fd), &options),
            )
            .await
    }
}

impl<'a> std::ops::Deref for Secret<'a> {
    type Target = zbus::Proxy<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A handy wrapper around [`Secret::retrieve`].
///
/// It crates a UnixStream internally for receiving the secret.
pub async fn retrieve() -> Result<Vec<u8>, Error> {
    let proxy = Secret::new().await?;
    let mut buf = Vec::with_capacity(64);

    #[cfg(feature = "tokio")]
    let mut x1 = {
        let (x1, mut x2) = UnixStream::pair()?;
        proxy.retrieve(&x2).await?;
        x2.shutdown().await?;
        x1
    };
    #[cfg(feature = "async-std")]
    let mut x1 = {
        let (x1, x2) = UnixStream::pair()?;
        proxy.retrieve(&x2).await?;
        x2.shutdown(Shutdown::Write)?;
        x1
    };

    x1.read_to_end(&mut buf).await?;

    Ok(buf)
}
