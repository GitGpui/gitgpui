//! Compose an email.
//!
//! Wrapper of the DBus interface: [`org.freedesktop.portal.Email`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Email.html).
//!
//! # Examples
//!
//! Compose an email
//!
//! ```rust,no_run
//! use std::{fs::File, os::fd::OwnedFd};
//!
//! use ashpd::desktop::email::EmailRequest;
//!
//! async fn run() -> ashpd::Result<()> {
//!     let file = File::open("/home/bilelmoussaoui/Downloads/adwaita-night.jpg").unwrap();
//!     EmailRequest::default()
//!         .address("test@gmail.com")
//!         .subject("email subject")
//!         .body("the pre-filled email body")
//!         .attach(OwnedFd::from(file))
//!         .send()
//!         .await;
//!     Ok(())
//! }
//! ```

use std::os::fd::OwnedFd;

use serde::Serialize;
use zbus::zvariant::{self, SerializeDict, Type};

use super::{HandleToken, Request};
use crate::{proxy::Proxy, ActivationToken, Error, WindowIdentifier};

#[derive(SerializeDict, Type, Debug, Default)]
#[zvariant(signature = "dict")]
struct EmailOptions {
    handle_token: HandleToken,
    address: Option<String>,
    addresses: Option<Vec<String>>,
    cc: Option<Vec<String>>,
    bcc: Option<Vec<String>>,
    subject: Option<String>,
    body: Option<String>,
    attachment_fds: Option<Vec<zvariant::OwnedFd>>,
    activation_token: Option<ActivationToken>,
}

#[derive(Debug)]
#[doc(alias = "org.freedesktop.portal.Email")]
struct EmailProxy<'a>(Proxy<'a>);

impl<'a> EmailProxy<'a> {
    /// Create a new instance of [`EmailProxy`].
    pub async fn new() -> Result<EmailProxy<'a>, Error> {
        let proxy = Proxy::new_desktop("org.freedesktop.portal.Email").await?;
        Ok(Self(proxy))
    }

    /// Presents a window that lets the user compose an email.
    ///
    /// **Note** the default email client for the host will need to support
    /// `mailto:` URIs following RFC 2368.
    ///
    /// # Arguments
    ///
    /// * `identifier` - Identifier for the application window.
    /// * `options` - An [`EmailOptions`].
    ///
    /// # Specifications
    ///
    /// See also [`ComposeEmail`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Email.html#org-freedesktop-portal-email-composeemail).
    #[doc(alias = "ComposeEmail")]
    pub async fn compose(
        &self,
        identifier: Option<&WindowIdentifier>,
        options: EmailOptions,
    ) -> Result<Request<()>, Error> {
        let identifier = identifier.map(|i| i.to_string()).unwrap_or_default();
        self.0
            .empty_request(
                &options.handle_token,
                "ComposeEmail",
                &(&identifier, &options),
            )
            .await
    }
}

impl<'a> std::ops::Deref for EmailProxy<'a> {
    type Target = zbus::Proxy<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Default)]
#[doc(alias = "xdp_portal_compose_email")]
/// A [builder-pattern] type to compose an email.
///
/// [builder-pattern]: https://doc.rust-lang.org/1.0.0/style/ownership/builders.html
pub struct EmailRequest {
    identifier: Option<WindowIdentifier>,
    options: EmailOptions,
}

impl EmailRequest {
    /// Sets a window identifier.
    #[must_use]
    pub fn identifier(mut self, identifier: impl Into<Option<WindowIdentifier>>) -> Self {
        self.identifier = identifier.into();
        self
    }

    /// Sets the email address to send the email to.
    #[must_use]
    pub fn address<'a>(mut self, address: impl Into<Option<&'a str>>) -> Self {
        self.options.address = address.into().map(ToOwned::to_owned);
        self
    }

    /// Sets a list of email addresses to send the email to.
    #[must_use]
    pub fn addresses<P: IntoIterator<Item = I>, I: AsRef<str> + Type + Serialize>(
        mut self,
        addresses: impl Into<Option<P>>,
    ) -> Self {
        self.options.addresses = addresses
            .into()
            .map(|a| a.into_iter().map(|s| s.as_ref().to_owned()).collect());
        self
    }

    /// Sets a list of email addresses to BCC.
    #[must_use]
    pub fn bcc<P: IntoIterator<Item = I>, I: AsRef<str> + Type + Serialize>(
        mut self,
        bcc: impl Into<Option<P>>,
    ) -> Self {
        self.options.bcc = bcc
            .into()
            .map(|a| a.into_iter().map(|s| s.as_ref().to_owned()).collect());
        self
    }

    /// Sets a list of email addresses to CC.
    #[must_use]
    pub fn cc<P: IntoIterator<Item = I>, I: AsRef<str> + Type + Serialize>(
        mut self,
        cc: impl Into<Option<P>>,
    ) -> Self {
        self.options.cc = cc
            .into()
            .map(|a| a.into_iter().map(|s| s.as_ref().to_owned()).collect());
        self
    }

    /// Sets the email subject.
    #[must_use]
    pub fn subject<'a>(mut self, subject: impl Into<Option<&'a str>>) -> Self {
        self.options.subject = subject.into().map(ToOwned::to_owned);
        self
    }

    /// Sets the email body.
    #[must_use]
    pub fn body<'a>(mut self, body: impl Into<Option<&'a str>>) -> Self {
        self.options.body = body.into().map(ToOwned::to_owned);
        self
    }

    /// Attaches a file to the email.
    #[must_use]
    pub fn attach(mut self, attachment: OwnedFd) -> Self {
        self.add_attachment(attachment);
        self
    }

    // TODO Added in version 4 of the interface.
    /// Sets the token that can be used to activate the chosen application.
    #[must_use]
    pub fn activation_token(
        mut self,
        activation_token: impl Into<Option<ActivationToken>>,
    ) -> Self {
        self.options.activation_token = activation_token.into();
        self
    }

    /// A different variant of [`Self::attach`].
    pub fn add_attachment(&mut self, attachment: OwnedFd) {
        let attachment = zvariant::OwnedFd::from(attachment);
        match self.options.attachment_fds {
            Some(ref mut attachments) => attachments.push(attachment),
            None => {
                self.options.attachment_fds.replace(vec![attachment]);
            }
        };
    }

    /// Send the request.
    pub async fn send(self) -> Result<Request<()>, Error> {
        let proxy = EmailProxy::new().await?;
        proxy.compose(self.identifier.as_ref(), self.options).await
    }
}
