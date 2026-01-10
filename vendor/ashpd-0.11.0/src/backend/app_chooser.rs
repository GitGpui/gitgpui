use std::sync::Arc;

use async_trait::async_trait;

use crate::{
    backend::{
        request::{Request, RequestImpl},
        MaybeAppID, MaybeWindowIdentifier,
    },
    desktop::{HandleToken, Response},
    zbus::object_server::{InterfaceRef, ObjectServer},
    zvariant::{DeserializeDict, OwnedObjectPath, SerializeDict, Type},
    ActivationToken, AppID, PortalError, WindowIdentifierType,
};

#[derive(Debug, DeserializeDict, Type)]
#[zvariant(signature = "dict")]
pub struct ChooserOptions {
    last_choice: Option<AppID>,
    modal: Option<bool>,
    content_type: Option<String>,
    uri: Option<url::Url>,
    filename: Option<String>,
    activation_token: Option<ActivationToken>,
}

impl ChooserOptions {
    pub fn last_choice(&self) -> Option<&AppID> {
        self.last_choice.as_ref()
    }

    pub fn modal(&self) -> Option<bool> {
        self.modal
    }

    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }

    pub fn uri(&self) -> Option<&url::Url> {
        self.uri.as_ref()
    }

    pub fn filename(&self) -> Option<&str> {
        self.filename.as_deref()
    }

    pub fn activation_token(&self) -> Option<&ActivationToken> {
        self.activation_token.as_ref()
    }
}

#[derive(Debug, SerializeDict, Type)]
#[zvariant(signature = "dict")]
pub struct Choice {
    choice: AppID,
    activation_token: Option<ActivationToken>,
}

impl Choice {
    pub fn new(choice: AppID) -> Self {
        Self {
            choice,
            activation_token: None,
        }
    }

    #[must_use]
    pub fn activation_token(
        mut self,
        activation_token: impl Into<Option<ActivationToken>>,
    ) -> Self {
        self.activation_token = activation_token.into();
        self
    }
}

#[async_trait]
pub trait AppChooserImpl: RequestImpl {
    async fn choose_application(
        &self,
        token: HandleToken,
        app_id: Option<AppID>,
        parent_window: Option<WindowIdentifierType>,
        choices: Vec<AppID>,
        options: ChooserOptions,
    ) -> Result<Choice, PortalError>;

    async fn update_choices(
        &self,
        request: InterfaceRef<Request>,
        choices: Vec<AppID>,
    ) -> Result<(), PortalError>;
}

pub(crate) struct AppChooserInterface {
    imp: Arc<dyn AppChooserImpl>,
    cnx: zbus::Connection,
}

impl AppChooserInterface {
    pub fn new(imp: Arc<dyn AppChooserImpl>, cnx: zbus::Connection) -> Self {
        Self { imp, cnx }
    }
}

#[zbus::interface(name = "org.freedesktop.impl.portal.AppChooser")]
impl AppChooserInterface {
    #[zbus(property(emits_changed_signal = "const"), name = "version")]
    fn version(&self) -> u32 {
        2
    }

    #[zbus(out_args("response", "results"))]
    async fn choose_application(
        &self,
        handle: OwnedObjectPath,
        app_id: MaybeAppID,
        parent_window: MaybeWindowIdentifier,
        choices: Vec<AppID>,
        options: ChooserOptions,
    ) -> Result<Response<Choice>, PortalError> {
        let imp = Arc::clone(&self.imp);

        Request::spawn(
            "AppChooser::ChooseApplication",
            &self.cnx,
            handle.clone(),
            Arc::clone(&self.imp),
            async move {
                imp.choose_application(
                    HandleToken::try_from(&handle).unwrap(),
                    app_id.inner(),
                    parent_window.inner(),
                    choices,
                    options,
                )
                .await
            },
        )
        .await
    }

    async fn update_choices(
        &self,
        #[zbus(object_server)] server: &ObjectServer,
        handle: OwnedObjectPath,
        choices: Vec<AppID>,
    ) -> Result<(), PortalError> {
        #[cfg(feature = "tracing")]
        tracing::debug!("AppChooser::UpdateChoices");

        let iface_ref = server.interface::<_, Request>(handle).await?;
        let response = self.imp.update_choices(iface_ref, choices).await;

        #[cfg(feature = "tracing")]
        tracing::debug!("AppChooser::UpdateChoices returned {:#?}", response);
        response
    }
}
