use std::sync::Arc;

use enumflags2::BitFlags;
use zbus::names::{OwnedWellKnownName, WellKnownName};

use crate::backend::{
    access::{AccessImpl, AccessInterface},
    account::{AccountImpl, AccountInterface},
    app_chooser::{AppChooserImpl, AppChooserInterface},
    background::{BackgroundImpl, BackgroundInterface},
    email::{EmailImpl, EmailInterface},
    file_chooser::{FileChooserImpl, FileChooserInterface},
    lockdown::{LockdownImpl, LockdownInterface},
    permission_store::{PermissionStoreImpl, PermissionStoreInterface},
    print::{PrintImpl, PrintInterface},
    screenshot::{ScreenshotImpl, ScreenshotInterface},
    secret::{SecretImpl, SecretInterface},
    settings::{SettingsImpl, SettingsInterface},
    wallpaper::{WallpaperImpl, WallpaperInterface},
    Result,
};

pub struct Builder {
    name: OwnedWellKnownName,
    flags: BitFlags<zbus::fdo::RequestNameFlags>,
    account_impl: Option<Arc<dyn AccountImpl>>,
    access_impl: Option<Arc<dyn AccessImpl>>,
    app_chooser_impl: Option<Arc<dyn AppChooserImpl>>,
    background_impl: Option<Arc<dyn BackgroundImpl>>,
    email_impl: Option<Arc<dyn EmailImpl>>,
    file_chooser_impl: Option<Arc<dyn FileChooserImpl>>,
    lockdown_impl: Option<Arc<dyn LockdownImpl>>,
    permission_store_impl: Option<Arc<dyn PermissionStoreImpl>>,
    print_impl: Option<Arc<dyn PrintImpl>>,
    screenshot_impl: Option<Arc<dyn ScreenshotImpl>>,
    secret_impl: Option<Arc<dyn SecretImpl>>,
    settings_impl: Option<Arc<dyn SettingsImpl>>,
    wallpaper_impl: Option<Arc<dyn WallpaperImpl>>,
}

impl Builder {
    pub fn new<'a, W>(well_known_name: W) -> zbus::Result<Self>
    where
        W: TryInto<WellKnownName<'a>>,
        <W as TryInto<WellKnownName<'a>>>::Error: Into<zbus::Error>,
    {
        let well_known_name = well_known_name.try_into().map_err(Into::into)?;
        Ok(Self {
            name: well_known_name.into(),
            // same flags as zbus::Connection::request_name
            flags: zbus::fdo::RequestNameFlags::ReplaceExisting
                | zbus::fdo::RequestNameFlags::DoNotQueue,
            account_impl: None,
            access_impl: None,
            app_chooser_impl: None,
            background_impl: None,
            email_impl: None,
            file_chooser_impl: None,
            lockdown_impl: None,
            permission_store_impl: None,
            print_impl: None,
            screenshot_impl: None,
            secret_impl: None,
            settings_impl: None,
            wallpaper_impl: None,
        })
    }

    pub fn with_flags(mut self, flags: BitFlags<zbus::fdo::RequestNameFlags>) -> Self {
        self.flags = flags;
        self
    }

    pub fn account(mut self, imp: impl AccountImpl + 'static) -> Self {
        self.account_impl = Some(Arc::new(imp));
        self
    }

    pub fn access(mut self, imp: impl AccessImpl + 'static) -> Self {
        self.access_impl = Some(Arc::new(imp));
        self
    }

    pub fn app_chooser(mut self, imp: impl AppChooserImpl + 'static) -> Self {
        self.app_chooser_impl = Some(Arc::new(imp));
        self
    }

    pub fn background(mut self, imp: impl BackgroundImpl + 'static) -> Self {
        self.background_impl = Some(Arc::new(imp));
        self
    }

    pub fn email(mut self, imp: impl EmailImpl + 'static) -> Self {
        self.email_impl = Some(Arc::new(imp));
        self
    }

    pub fn file_chooser(mut self, imp: impl FileChooserImpl + 'static) -> Self {
        self.file_chooser_impl = Some(Arc::new(imp));
        self
    }

    pub fn lockdown(mut self, imp: impl LockdownImpl + 'static) -> Self {
        self.lockdown_impl = Some(Arc::new(imp));
        self
    }

    pub fn permission_store(mut self, imp: impl PermissionStoreImpl + 'static) -> Self {
        self.permission_store_impl = Some(Arc::new(imp));
        self
    }

    pub fn print(mut self, imp: impl PrintImpl + 'static) -> Self {
        self.print_impl = Some(Arc::new(imp));
        self
    }

    pub fn screenshot(mut self, imp: impl ScreenshotImpl + 'static) -> Self {
        self.screenshot_impl = Some(Arc::new(imp));
        self
    }

    pub fn secret(mut self, imp: impl SecretImpl + 'static) -> Self {
        self.secret_impl = Some(Arc::new(imp));
        self
    }

    pub fn settings(mut self, imp: impl SettingsImpl + 'static) -> Self {
        self.settings_impl = Some(Arc::new(imp));
        self
    }

    pub fn wallpaper(mut self, imp: impl WallpaperImpl + 'static) -> Self {
        self.wallpaper_impl = Some(Arc::new(imp));
        self
    }

    pub async fn build(self) -> Result<()> {
        let cnx = zbus::Connection::session().await?;
        cnx.request_name_with_flags(self.name, self.flags).await?;
        let object_server = cnx.object_server();
        if let Some(imp) = self.account_impl {
            let portal = AccountInterface::new(imp, cnx.clone());
            #[cfg(feature = "tracing")]
            tracing::debug!("Serving interface `org.freedesktop.impl.portal.Account`");
            object_server
                .at("/org/freedesktop/portal/desktop", portal)
                .await?;
        }

        if let Some(imp) = self.access_impl {
            let portal = AccessInterface::new(imp, cnx.clone());
            #[cfg(feature = "tracing")]
            tracing::debug!("Serving interface `org.freedesktop.impl.portal.Access`");
            object_server
                .at("/org/freedesktop/portal/desktop", portal)
                .await?;
        }

        if let Some(imp) = self.app_chooser_impl {
            let portal = AppChooserInterface::new(imp, cnx.clone());
            #[cfg(feature = "tracing")]
            tracing::debug!("Serving interface `org.freedesktop.impl.portal.AppChooser`");
            object_server
                .at("/org/freedesktop/portal/desktop", portal)
                .await?;
        }

        if let Some(imp) = self.background_impl {
            let portal = BackgroundInterface::new(imp, cnx.clone());
            #[cfg(feature = "tracing")]
            tracing::debug!("Serving interface `org.freedesktop.impl.portal.Background`");
            object_server
                .at("/org/freedesktop/portal/desktop", portal)
                .await?;
        }

        if let Some(imp) = self.email_impl {
            let portal = EmailInterface::new(imp, cnx.clone());
            #[cfg(feature = "tracing")]
            tracing::debug!("Serving interface `org.freedesktop.impl.portal.Email`");
            object_server
                .at("/org/freedesktop/portal/desktop", portal)
                .await?;
        }

        if let Some(imp) = self.file_chooser_impl {
            let portal = FileChooserInterface::new(imp, cnx.clone());
            #[cfg(feature = "tracing")]
            tracing::debug!("Serving interface `org.freedesktop.impl.portal.FileChooser`");
            object_server
                .at("/org/freedesktop/portal/desktop", portal)
                .await?;
        }

        if let Some(imp) = self.lockdown_impl {
            let portal = LockdownInterface::new(imp, cnx.clone());
            #[cfg(feature = "tracing")]
            tracing::debug!("Serving interface `org.freedesktop.impl.portal.Lockdown`");
            object_server
                .at("/org/freedesktop/portal/desktop", portal)
                .await?;
        }

        if let Some(imp) = self.permission_store_impl {
            let portal = PermissionStoreInterface::new(imp, cnx.clone());
            #[cfg(feature = "tracing")]
            tracing::debug!("Serving interface `org.freedesktop.impl.portal.PermissionStore`");
            object_server
                .at("/org/freedesktop/portal/desktop", portal)
                .await?;
        }

        if let Some(imp) = self.print_impl {
            let portal = PrintInterface::new(imp, cnx.clone());
            #[cfg(feature = "tracing")]
            tracing::debug!("Serving interface `org.freedesktop.impl.portal.Print`");
            object_server
                .at("/org/freedesktop/portal/desktop", portal)
                .await?;
        }

        if let Some(imp) = self.screenshot_impl {
            let portal = ScreenshotInterface::new(imp, cnx.clone());
            #[cfg(feature = "tracing")]
            tracing::debug!("Serving interface `org.freedesktop.impl.portal.Screenshot`");
            object_server
                .at("/org/freedesktop/portal/desktop", portal)
                .await?;
        }

        if let Some(imp) = self.secret_impl {
            let portal = SecretInterface::new(imp, cnx.clone());
            #[cfg(feature = "tracing")]
            tracing::debug!("Serving interface `org.freedesktop.impl.portal.Secret`");
            object_server
                .at("/org/freedesktop/portal/desktop", portal)
                .await?;
        }

        if let Some(imp) = self.settings_impl {
            let portal = SettingsInterface::new(imp, cnx.clone());
            #[cfg(feature = "tracing")]
            tracing::debug!("Serving interface `org.freedesktop.impl.portal.Settings`");
            object_server
                .at("/org/freedesktop/portal/desktop", portal)
                .await?;
        }

        if let Some(imp) = self.wallpaper_impl {
            let portal = WallpaperInterface::new(imp, cnx.clone());
            #[cfg(feature = "tracing")]
            tracing::debug!("Serving interface `org.freedesktop.impl.portal.Wallpaper`");
            object_server
                .at("/org/freedesktop/portal/desktop", portal)
                .await?;
        }

        Ok(())
    }
}
