use std::sync::Arc;

use async_trait::async_trait;

use crate::{
    backend::{
        request::{Request, RequestImpl},
        MaybeAppID, MaybeWindowIdentifier, Result,
    },
    desktop::{file_chooser::Choice, request::Response, HandleToken, Icon},
    zvariant::{self, DeserializeDict, OwnedObjectPath, SerializeDict},
    AppID, WindowIdentifierType,
};

#[derive(DeserializeDict, zvariant::Type)]
#[zvariant(signature = "dict")]
pub struct AccessOptions {
    modal: Option<bool>,
    deny_label: Option<String>,
    grant_label: Option<String>,
    icon: Option<String>,
    choices: Option<Vec<Choice>>,
}

impl AccessOptions {
    pub fn is_modal(&self) -> Option<bool> {
        self.modal
    }

    pub fn deny_label(&self) -> Option<&str> {
        self.deny_label.as_deref()
    }

    pub fn grant_label(&self) -> Option<&str> {
        self.grant_label.as_deref()
    }

    pub fn icon(&self) -> Option<Icon> {
        self.icon.as_ref().map(|i| Icon::with_names([i]))
    }

    pub fn choices(&self) -> &[Choice] {
        self.choices.as_deref().unwrap_or_default()
    }
}

#[derive(SerializeDict, Debug, zvariant::Type, Default)]
#[zvariant(signature = "dict")]
pub struct AccessResponse {
    choices: Option<Vec<(String, String)>>,
}

impl AccessResponse {
    /// Adds a selected choice (key, value).
    #[must_use]
    pub fn choice(mut self, key: &str, value: &str) -> Self {
        self.choices
            .get_or_insert_with(Vec::new)
            .push((key.to_owned(), value.to_owned()));
        self
    }
}

#[async_trait]
pub trait AccessImpl: RequestImpl {
    #[allow(clippy::too_many_arguments)]
    async fn access_dialog(
        &self,
        token: HandleToken,
        app_id: Option<AppID>,
        window_identifier: Option<WindowIdentifierType>,
        title: String,
        subtitle: String,
        body: String,
        options: AccessOptions,
    ) -> Result<AccessResponse>;
}

pub(crate) struct AccessInterface {
    imp: Arc<dyn AccessImpl>,
    cnx: zbus::Connection,
}

impl AccessInterface {
    pub fn new(imp: Arc<dyn AccessImpl>, cnx: zbus::Connection) -> Self {
        Self { imp, cnx }
    }
}

#[zbus::interface(name = "org.freedesktop.impl.portal.Access")]
impl AccessInterface {
    #[zbus(property(emits_changed_signal = "const"), name = "version")]
    fn version(&self) -> u32 {
        1 // TODO: Is this correct?
    }

    #[allow(clippy::too_many_arguments)]
    #[zbus(out_args("response", "results"))]
    async fn access_dialog(
        &self,
        handle: OwnedObjectPath,
        app_id: MaybeAppID,
        window_identifier: MaybeWindowIdentifier,
        title: String,
        subtitle: String,
        body: String,
        options: AccessOptions,
    ) -> Result<Response<AccessResponse>> {
        let imp = Arc::clone(&self.imp);

        Request::spawn(
            "Access::AccessDialog",
            &self.cnx,
            handle.clone(),
            Arc::clone(&self.imp),
            async move {
                imp.access_dialog(
                    HandleToken::try_from(&handle).unwrap(),
                    app_id.inner(),
                    window_identifier.inner(),
                    title,
                    subtitle,
                    body,
                    options,
                )
                .await
            },
        )
        .await
    }
}
