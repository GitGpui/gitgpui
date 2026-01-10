use std::sync::Arc;

use async_trait::async_trait;

use crate::{
    backend::{
        request::{Request, RequestImpl},
        MaybeAppID, MaybeWindowIdentifier, Result,
    },
    desktop::{
        request::Response, screenshot::Screenshot as ScreenshotResponse, Color, HandleToken,
    },
    zvariant::{DeserializeDict, OwnedObjectPath, Type},
    AppID, WindowIdentifierType,
};

#[derive(DeserializeDict, Type, Debug)]
#[zvariant(signature = "dict")]
pub struct ScreenshotOptions {
    modal: Option<bool>,
    interactive: Option<bool>,
    permission_store_checked: Option<bool>,
}

impl ScreenshotOptions {
    pub fn modal(&self) -> Option<bool> {
        self.modal
    }

    pub fn interactive(&self) -> Option<bool> {
        self.interactive
    }

    pub fn permission_store_checked(&self) -> Option<bool> {
        self.permission_store_checked
    }
}

#[derive(DeserializeDict, Type, Debug)]
#[zvariant(signature = "dict")]
pub struct ColorOptions;

#[async_trait]
pub trait ScreenshotImpl: RequestImpl {
    async fn screenshot(
        &self,
        token: HandleToken,
        app_id: Option<AppID>,
        window_identifier: Option<WindowIdentifierType>,
        options: ScreenshotOptions,
    ) -> Result<ScreenshotResponse>;

    async fn pick_color(
        &self,
        token: HandleToken,
        app_id: Option<AppID>,
        window_identifier: Option<WindowIdentifierType>,
        options: ColorOptions,
    ) -> Result<Color>;
}

pub(crate) struct ScreenshotInterface {
    imp: Arc<dyn ScreenshotImpl>,
    cnx: zbus::Connection,
}

impl ScreenshotInterface {
    pub fn new(imp: Arc<dyn ScreenshotImpl>, cnx: zbus::Connection) -> Self {
        Self { imp, cnx }
    }
}

#[zbus::interface(name = "org.freedesktop.impl.portal.Screenshot")]
impl ScreenshotInterface {
    #[zbus(property(emits_changed_signal = "const"), name = "version")]
    fn version(&self) -> u32 {
        2
    }

    #[zbus(name = "Screenshot")]
    #[zbus(out_args("response", "results"))]
    async fn screenshot(
        &self,
        handle: OwnedObjectPath,
        app_id: MaybeAppID,
        window_identifier: MaybeWindowIdentifier,
        options: ScreenshotOptions,
    ) -> Result<Response<ScreenshotResponse>> {
        let imp = Arc::clone(&self.imp);

        Request::spawn(
            "Screenshot::Screenshot",
            &self.cnx,
            handle.clone(),
            Arc::clone(&self.imp),
            async move {
                imp.screenshot(
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

    #[zbus(name = "PickColor")]
    #[zbus(out_args("response", "results"))]
    async fn pick_color(
        &self,
        handle: OwnedObjectPath,
        app_id: MaybeAppID,
        window_identifier: MaybeWindowIdentifier,
        options: ColorOptions,
    ) -> Result<Response<Color>> {
        let imp = Arc::clone(&self.imp);

        Request::spawn(
            "Screenshot::PickColor",
            &self.cnx,
            handle.clone(),
            Arc::clone(&self.imp),
            async move {
                imp.pick_color(
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
