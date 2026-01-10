use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use zbus::zvariant::{self, OwnedValue};

use crate::{
    backend::{
        request::{Request, RequestImpl},
        Result,
    },
    desktop::{HandleToken, Response},
    AppID,
};

#[async_trait]
pub trait SecretImpl: RequestImpl {
    async fn retrieve(
        &self,
        token: HandleToken,
        app_id: AppID,
        fd: std::os::fd::OwnedFd,
    ) -> Result<HashMap<String, OwnedValue>>;
}

pub(crate) struct SecretInterface {
    imp: Arc<dyn SecretImpl>,
    cnx: zbus::Connection,
}

impl SecretInterface {
    pub fn new(imp: Arc<dyn SecretImpl>, cnx: zbus::Connection) -> Self {
        Self { imp, cnx }
    }
}

#[zbus::interface(name = "org.freedesktop.impl.portal.Secret")]
impl SecretInterface {
    #[zbus(property(emits_changed_signal = "const"), name = "version")]
    fn version(&self) -> u32 {
        1
    }

    #[zbus(out_args("response", "results"))]
    async fn retrieve_secret(
        &self,
        handle: zvariant::OwnedObjectPath,
        app_id: AppID,
        fd: zvariant::OwnedFd,
        _options: HashMap<String, OwnedValue>,
    ) -> Result<Response<HashMap<String, OwnedValue>>> {
        let imp = Arc::clone(&self.imp);

        Request::spawn(
            "Secret::RetrieveSecret",
            &self.cnx,
            handle.clone(),
            Arc::clone(&self.imp),
            async move {
                imp.retrieve(
                    HandleToken::try_from(&handle).unwrap(),
                    app_id,
                    std::os::fd::OwnedFd::from(fd),
                )
                .await
            },
        )
        .await
    }
}
