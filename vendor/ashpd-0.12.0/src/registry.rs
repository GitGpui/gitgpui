use zbus::zvariant;

use crate::{proxy::Proxy, AppID, Error};

#[derive(Debug, zvariant::SerializeDict, zvariant::Type)]
#[zvariant(signature = "dict")]
struct RegisterOptions {}

struct RegistryProxy<'a>(Proxy<'a>);

impl<'a> RegistryProxy<'a> {
    pub async fn new() -> Result<RegistryProxy<'a>, Error> {
        let proxy = Proxy::new_desktop("org.freedesktop.host.portal.Registry").await?;
        Ok(Self(proxy))
    }

    pub async fn register(&self, app_id: AppID) -> Result<(), Error> {
        let options = RegisterOptions {};
        self.0.call_method("Register", &(&app_id, &options)).await?;
        Ok(())
    }
}

impl<'a> std::ops::Deref for RegistryProxy<'a> {
    type Target = zbus::Proxy<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
/// Registers a host application for portal usage.
///
/// Portals rely on the application ID to store and manage the permissions of
/// individual apps. However, for non-sandboxed (host) applications, this
/// information cannot be directly retrieved. To resolve this, the method first
/// verifies that the application is not sandboxed. It then uses the
/// `org.freedesktop.host.portal.Registry` interface to register the process
/// communicating over DBus with the portal as the owner of the specified
/// application ID.
/// For more technical details, see <https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.host.portal.Registry.html>
pub async fn register_host_app(app_id: AppID) -> crate::Result<()> {
    if crate::is_sandboxed().await {
        return Ok(());
    }
    let proxy = RegistryProxy::new().await?;
    proxy.register(app_id).await?;
    Ok(())
}
