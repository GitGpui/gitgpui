use std::sync::Arc;

use async_trait::async_trait;

use crate::{
    backend::{
        request::{Request, RequestImpl},
        MaybeAppID, MaybeWindowIdentifier, Result,
    },
    desktop::{request::Response, HandleToken},
    zvariant::{self, DeserializeDict, OwnedObjectPath},
    ActivationToken, AppID, WindowIdentifierType,
};

#[derive(DeserializeDict, zvariant::Type)]
#[zvariant(signature = "dict")]
pub struct Options {
    address: Option<String>,
    addresses: Option<Vec<String>>,
    cc: Option<Vec<String>>,
    bcc: Option<Vec<String>>,
    subject: Option<String>,
    body: Option<String>,
    attachments: Option<Vec<url::Url>>,
    activation_token: Option<ActivationToken>,
}

impl Options {
    pub fn address(&self) -> Option<&str> {
        self.address.as_deref()
    }

    pub fn addresses(&self) -> &[String] {
        self.addresses.as_deref().unwrap_or_default()
    }

    pub fn cc(&self) -> &[String] {
        self.cc.as_deref().unwrap_or_default()
    }

    pub fn bcc(&self) -> &[String] {
        self.bcc.as_deref().unwrap_or_default()
    }

    pub fn subject(&self) -> Option<&str> {
        self.subject.as_deref()
    }

    pub fn body(&self) -> Option<&str> {
        self.body.as_deref()
    }

    pub fn attachments(&self) -> &[url::Url] {
        self.attachments.as_deref().unwrap_or_default()
    }

    pub fn activation_token(&self) -> Option<&ActivationToken> {
        self.activation_token.as_ref()
    }
}

#[async_trait]
pub trait EmailImpl: RequestImpl {
    async fn compose(
        &self,
        token: HandleToken,
        app_id: Option<AppID>,
        window_identifier: Option<WindowIdentifierType>,
        options: Options,
    ) -> Result<()>;
}

pub(crate) struct EmailInterface {
    imp: Arc<dyn EmailImpl>,
    cnx: zbus::Connection,
}

impl EmailInterface {
    pub fn new(imp: Arc<dyn EmailImpl>, cnx: zbus::Connection) -> Self {
        Self { imp, cnx }
    }
}

#[zbus::interface(name = "org.freedesktop.impl.portal.Email")]
impl EmailInterface {
    #[zbus(property(emits_changed_signal = "const"), name = "version")]
    fn version(&self) -> u32 {
        4
    }

    #[zbus(out_args("response", "results"))]
    async fn compose_email(
        &self,
        handle: OwnedObjectPath,
        app_id: MaybeAppID,
        window_identifier: MaybeWindowIdentifier,
        options: Options,
    ) -> Result<Response<()>> {
        let imp = Arc::clone(&self.imp);

        Request::spawn(
            "Email::ComposeEmail",
            &self.cnx,
            handle.clone(),
            Arc::clone(&self.imp),
            async move {
                imp.compose(
                    HandleToken::try_from(&handle).unwrap(),
                    app_id.inner(),
                    window_identifier.inner(),
                    options,
                )
                .await
            },
        )
        .await
    }
}
