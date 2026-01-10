//! Take a screenshot or pick a color.
//!
//! Wrapper of the DBus interface: [`org.freedesktop.portal.Screenshot`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Screenshot.html).
//!
//! # Examples
//!
//! ## Taking a screenshot
//!
//! ```rust,no_run
//! use ashpd::desktop::screenshot::Screenshot;
//!
//! async fn run() -> ashpd::Result<()> {
//!     let response = Screenshot::request()
//!         .interactive(true)
//!         .modal(true)
//!         .send()
//!         .await?
//!         .response()?;
//!     println!("URI: {}", response.uri());
//!     Ok(())
//! }
//! ```
//!
//! ## Picking a color
//!
//! ```rust,no_run
//! use ashpd::desktop::Color;
//!
//! async fn run() -> ashpd::Result<()> {
//!     let color = Color::pick().send().await?.response()?;
//!     println!("({}, {}, {})", color.red(), color.green(), color.blue());
//!
//!     Ok(())
//! }
//! ```
use std::fmt::Debug;

use zbus::zvariant::{DeserializeDict, SerializeDict, Type};

use super::{HandleToken, Request};
use crate::{desktop::Color, proxy::Proxy, Error, WindowIdentifier};

#[derive(SerializeDict, Type, Debug, Default)]
#[zvariant(signature = "dict")]
struct ScreenshotOptions {
    handle_token: HandleToken,
    modal: Option<bool>,
    interactive: Option<bool>,
}

#[derive(SerializeDict, DeserializeDict, Type)]
#[zvariant(signature = "dict")]
/// The response of a [`ScreenshotRequest`] request.
pub struct Screenshot {
    uri: url::Url,
}

impl Screenshot {
    #[cfg(feature = "backend")]
    #[cfg_attr(docsrs, doc(cfg(feature = "backend")))]
    /// Create a new instance of the screenshot.
    pub fn new(uri: url::Url) -> Self {
        Self { uri }
    }

    /// Creates a new builder-pattern struct instance to construct
    /// [`Screenshot`].
    ///
    /// This method returns an instance of [`ScreenshotRequest`].
    pub fn request() -> ScreenshotRequest {
        ScreenshotRequest::default()
    }

    /// The screenshot URI.
    pub fn uri(&self) -> &url::Url {
        &self.uri
    }
}

impl Debug for Screenshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.uri.as_str())
    }
}

#[derive(SerializeDict, Type, Debug, Default)]
#[zvariant(signature = "dict")]
struct ColorOptions {
    handle_token: HandleToken,
}

#[derive(Debug)]
#[doc(alias = "org.freedesktop.portal.Screenshot")]
struct ScreenshotProxy<'a>(Proxy<'a>);

impl<'a> ScreenshotProxy<'a> {
    /// Create a new instance of [`ScreenshotProxy`].
    pub async fn new() -> Result<ScreenshotProxy<'a>, Error> {
        let proxy = Proxy::new_desktop("org.freedesktop.portal.Screenshot").await?;
        Ok(Self(proxy))
    }

    /// Obtains the color of a single pixel.
    ///
    /// # Arguments
    ///
    /// * `identifier` - Identifier for the application window.
    ///
    /// # Specifications
    ///
    /// See also [`PickColor`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Screenshot.html#org-freedesktop-portal-screenshot-pickcolor).
    #[doc(alias = "PickColor")]
    #[doc(alias = "xdp_portal_pick_color")]
    pub async fn pick_color(
        &self,
        identifier: Option<&WindowIdentifier>,
        options: ColorOptions,
    ) -> Result<Request<Color>, Error> {
        let identifier = identifier.map(|i| i.to_string()).unwrap_or_default();
        self.0
            .request(&options.handle_token, "PickColor", &(&identifier, &options))
            .await
    }

    /// Takes a screenshot.
    ///
    /// # Arguments
    ///
    /// * `identifier` - Identifier for the application window.
    /// * `interactive` - Sets whether the dialog should offer customization
    ///   before a screenshot or not.
    /// * `modal` - Sets whether the dialog should be a modal.
    ///
    /// # Returns
    ///
    /// The screenshot URI.
    ///
    /// # Specifications
    ///
    /// See also [`Screenshot`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Screenshot.html#org-freedesktop-portal-screenshot-screenshot).
    #[doc(alias = "Screenshot")]
    #[doc(alias = "xdp_portal_take_screenshot")]
    pub async fn screenshot(
        &self,
        identifier: Option<&WindowIdentifier>,
        options: ScreenshotOptions,
    ) -> Result<Request<Screenshot>, Error> {
        let identifier = identifier.map(|i| i.to_string()).unwrap_or_default();
        self.0
            .request(
                &options.handle_token,
                "Screenshot",
                &(&identifier, &options),
            )
            .await
    }
}

impl<'a> std::ops::Deref for ScreenshotProxy<'a> {
    type Target = zbus::Proxy<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Default)]
#[doc(alias = "xdp_portal_pick_color")]
/// A [builder-pattern] type to construct [`Color`].
///
/// [builder-pattern]: https://doc.rust-lang.org/1.0.0/style/ownership/builders.html
pub struct ColorRequest {
    identifier: Option<WindowIdentifier>,
    options: ColorOptions,
}

impl ColorRequest {
    #[must_use]
    /// Sets a window identifier.
    pub fn identifier(mut self, identifier: impl Into<Option<WindowIdentifier>>) -> Self {
        self.identifier = identifier.into();
        self
    }

    /// Build the [`Color`].
    pub async fn send(self) -> Result<Request<Color>, Error> {
        let proxy = ScreenshotProxy::new().await?;
        proxy
            .pick_color(self.identifier.as_ref(), self.options)
            .await
    }
}

impl Color {
    /// Creates a new builder-pattern struct instance to construct
    /// [`Color`].
    ///
    /// This method returns an instance of [`ColorRequest`].
    pub fn pick() -> ColorRequest {
        ColorRequest::default()
    }
}

#[derive(Debug, Default)]
#[doc(alias = "xdp_portal_take_screenshot")]
/// A [builder-pattern] type to construct a screenshot [`Screenshot`].
///
/// [builder-pattern]: https://doc.rust-lang.org/1.0.0/style/ownership/builders.html
pub struct ScreenshotRequest {
    options: ScreenshotOptions,
    identifier: Option<WindowIdentifier>,
}

impl ScreenshotRequest {
    #[must_use]
    /// Sets a window identifier.
    pub fn identifier(mut self, identifier: impl Into<Option<WindowIdentifier>>) -> Self {
        self.identifier = identifier.into();
        self
    }

    /// Sets whether the dialog should be a modal.
    #[must_use]
    pub fn modal(mut self, modal: impl Into<Option<bool>>) -> Self {
        self.options.modal = modal.into();
        self
    }

    /// Sets whether the dialog should offer customization before a screenshot
    /// or not.
    #[must_use]
    pub fn interactive(mut self, interactive: impl Into<Option<bool>>) -> Self {
        self.options.interactive = interactive.into();
        self
    }

    /// Build the [`Screenshot`].
    pub async fn send(self) -> Result<Request<Screenshot>, Error> {
        let proxy = ScreenshotProxy::new().await?;
        proxy
            .screenshot(self.identifier.as_ref(), self.options)
            .await
    }
}
