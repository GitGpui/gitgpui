use std::sync::Arc;

use async_trait::async_trait;

use crate::{
    backend::{
        request::{Request, RequestImpl},
        MaybeAppID, MaybeWindowIdentifier, Result,
    },
    desktop::{request::Response, usb::UsbDevice, HandleToken},
    zvariant::{DeserializeDict, OwnedObjectPath, SerializeDict, Type},
    AppID, WindowIdentifierType,
};

#[derive(Debug, DeserializeDict, Type)]
#[zvariant(signature = "dict")]
pub struct AcquireDevicesOptions {}

#[derive(Debug, SerializeDict, DeserializeDict, Type)]
#[zvariant(signature = "dict")]
pub struct AccessOptions {
    writable: Option<bool>,
}

impl AccessOptions {
    pub fn new(is_writable: bool) -> Self {
        Self {
            writable: Some(is_writable),
        }
    }

    pub fn is_writable(&self) -> Option<bool> {
        self.writable
    }
}

#[async_trait]
pub trait UsbImpl: RequestImpl {
    async fn acquire_devices(
        &self,
        token: HandleToken,
        window_identifier: Option<WindowIdentifierType>,
        app_id: Option<AppID>,
        devices: Vec<(String, UsbDevice, AccessOptions)>,
        options: AcquireDevicesOptions,
    ) -> Result<Vec<(String, AccessOptions)>>;
}

pub(crate) struct UsbInterface {
    imp: Arc<dyn UsbImpl>,
    spawn: Arc<dyn futures_util::task::Spawn + Send + Sync>,
    cnx: zbus::Connection,
}

impl UsbInterface {
    pub fn new(
        imp: Arc<dyn UsbImpl>,
        cnx: zbus::Connection,
        spawn: Arc<dyn futures_util::task::Spawn + Send + Sync>,
    ) -> Self {
        Self { imp, cnx, spawn }
    }
}

#[zbus::interface(name = "org.freedesktop.impl.portal.Usb")]
impl UsbInterface {
    #[zbus(property(emits_changed_signal = "const"), name = "version")]
    fn version(&self) -> u32 {
        1
    }

    #[zbus(name = "AcquireDevices")]
    #[zbus(out_args("response", "results"))]
    async fn acquire_devices(
        &self,
        handle: OwnedObjectPath,
        window_identifier: MaybeWindowIdentifier,
        app_id: MaybeAppID,
        devices: Vec<(String, UsbDevice, AccessOptions)>,
        options: AcquireDevicesOptions,
    ) -> Result<Response<Vec<(String, AccessOptions)>>> {
        let imp = Arc::clone(&self.imp);

        Request::spawn(
            "Usb::AcquireDevices",
            &self.cnx,
            handle.clone(),
            Arc::clone(&self.imp),
            Arc::clone(&self.spawn),
            async move {
                imp.acquire_devices(
                    HandleToken::try_from(&handle).unwrap(),
                    window_identifier.inner(),
                    app_id.inner(),
                    devices,
                    options,
                )
                .await
            },
        )
        .await
    }
}
