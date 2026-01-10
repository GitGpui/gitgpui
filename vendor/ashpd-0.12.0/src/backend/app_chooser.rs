use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Deserializer};

use crate::{
    backend::{
        request::{Request, RequestImpl},
        MaybeAppID, MaybeWindowIdentifier,
    },
    desktop::{HandleToken, Response},
    zvariant::{DeserializeDict, OwnedObjectPath, SerializeDict, Type},
    ActivationToken, AppID, PortalError, WindowIdentifierType,
};

/// The desktop ID of an application.
///
/// This is the name of application's desktop entry file without the `.desktop`
/// suffix. This ID may or may not follow the [application ID
/// guidelines](https://developer.gnome.org/documentation/tutorials/application-id.html).
#[derive(Debug, Type)]
#[zvariant(signature = "s")]
pub struct DesktopID(Result<AppID, String>);

impl DesktopID {
    pub fn inner(&self) -> &Result<AppID, String> {
        &self.0
    }
}

impl<'de> Deserialize<'de> for DesktopID {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let inner = String::deserialize(deserializer)?;
        Ok(Self(inner.parse::<AppID>().or(Err(inner))))
    }
}

impl std::fmt::Display for DesktopID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Ok(app_id) => f.write_str(app_id),
            Err(app_id) => f.write_str(app_id),
        }
    }
}

#[derive(Debug, DeserializeDict, Type)]
#[zvariant(signature = "dict")]
pub struct ChooserOptions {
    last_choice: Option<DesktopID>,
    modal: Option<bool>,
    content_type: Option<String>,
    uri: Option<url::Url>,
    filename: Option<String>,
    activation_token: Option<ActivationToken>,
}

impl ChooserOptions {
    pub fn last_choice(&self) -> Option<&DesktopID> {
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
        choices: Vec<DesktopID>,
        options: ChooserOptions,
    ) -> Result<Choice, PortalError>;

    async fn update_choices(
        &self,
        token: HandleToken,
        choices: Vec<DesktopID>,
    ) -> Result<(), PortalError>;
}

pub(crate) struct AppChooserInterface {
    imp: Arc<dyn AppChooserImpl>,
    spawn: Arc<dyn futures_util::task::Spawn + Send + Sync>,
    cnx: zbus::Connection,
}

impl AppChooserInterface {
    pub fn new(
        imp: Arc<dyn AppChooserImpl>,
        cnx: zbus::Connection,
        spawn: Arc<dyn futures_util::task::Spawn + Send + Sync>,
    ) -> Self {
        Self { imp, cnx, spawn }
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
        choices: Vec<DesktopID>,
        options: ChooserOptions,
    ) -> Result<Response<Choice>, PortalError> {
        let imp = Arc::clone(&self.imp);

        Request::spawn(
            "AppChooser::ChooseApplication",
            &self.cnx,
            handle.clone(),
            Arc::clone(&self.imp),
            Arc::clone(&self.spawn),
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
        handle: OwnedObjectPath,
        choices: Vec<DesktopID>,
    ) -> Result<(), PortalError> {
        #[cfg(feature = "tracing")]
        tracing::debug!("AppChooser::UpdateChoices");

        let token = HandleToken::try_from(&handle).unwrap();
        let response = self.imp.update_choices(token, choices).await;

        #[cfg(feature = "tracing")]
        tracing::debug!("AppChooser::UpdateChoices returned {:#?}", response);
        response
    }
}
