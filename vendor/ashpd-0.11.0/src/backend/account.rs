use std::sync::Arc;

use async_trait::async_trait;

use crate::{
    backend::{
        request::{Request, RequestImpl},
        MaybeAppID, MaybeWindowIdentifier, Result,
    },
    desktop::{account::UserInformation, request::Response, HandleToken},
    zvariant::{DeserializeDict, OwnedObjectPath, Type},
    AppID, WindowIdentifierType,
};

#[derive(Debug, DeserializeDict, Type)]
#[zvariant(signature = "dict")]
pub struct UserInformationOptions {
    reason: Option<String>,
}

impl UserInformationOptions {
    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

#[async_trait]
pub trait AccountImpl: RequestImpl {
    async fn get_user_information(
        &self,
        token: HandleToken,
        app_id: Option<AppID>,
        window_identifier: Option<WindowIdentifierType>,
        options: UserInformationOptions,
    ) -> Result<UserInformation>;
}

pub(crate) struct AccountInterface {
    imp: Arc<dyn AccountImpl>,
    cnx: zbus::Connection,
}

impl AccountInterface {
    pub fn new(imp: Arc<dyn AccountImpl>, cnx: zbus::Connection) -> Self {
        Self { imp, cnx }
    }
}

#[zbus::interface(name = "org.freedesktop.impl.portal.Account")]
impl AccountInterface {
    #[zbus(property(emits_changed_signal = "const"), name = "version")]
    fn version(&self) -> u32 {
        1
    }

    #[zbus(name = "GetUserInformation")]
    #[zbus(out_args("response", "results"))]
    async fn get_user_information(
        &self,
        handle: OwnedObjectPath,
        app_id: MaybeAppID,
        window_identifier: MaybeWindowIdentifier,
        options: UserInformationOptions,
    ) -> Result<Response<UserInformation>> {
        let imp = Arc::clone(&self.imp);

        Request::spawn(
            "Account::GetUserInformation",
            &self.cnx,
            handle.clone(),
            Arc::clone(&self.imp),
            async move {
                imp.get_user_information(
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
