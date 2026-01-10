mod handle_token;
pub(crate) mod request;
mod session;
#[cfg_attr(docsrs, doc(cfg(feature = "backend")))]
#[cfg(feature = "backend")]
pub use self::handle_token::HandleToken;
#[cfg(not(feature = "backend"))]
pub(crate) use self::handle_token::HandleToken;
pub use self::{
    request::{Request, Response, ResponseError, ResponseType},
    session::{Session, SessionPortal},
};
mod color;
pub use color::Color;
mod icon;
pub use icon::Icon;

pub mod account;
pub mod background;
pub mod camera;
pub mod clipboard;
#[deprecated = "The portal does not serve any purpose as nothing really can make use of it as is."]
pub mod device;
pub mod dynamic_launcher;
pub mod email;
/// Open/save file(s) chooser.
pub mod file_chooser;
/// Enable/disable/query the status of Game Mode.
pub mod game_mode;
/// Register global shortcuts
pub mod global_shortcuts;
/// Inhibit the session from being restarted or the user from logging out.
pub mod inhibit;
/// Capture input events from physical or logical devices.
pub mod input_capture;
/// Query the user's GPS location.
pub mod location;
/// Monitor memory level.
pub mod memory_monitor;
/// Check the status of the network on a user's machine.
pub mod network_monitor;
/// Send/withdraw notifications.
pub mod notification;
pub mod open_uri;
/// Power profile monitoring.
pub mod power_profile_monitor;
/// Print a document.
pub mod print;
/// Proxy information.
pub mod proxy_resolver;
pub mod realtime;
/// Start a remote desktop session and interact with it.
pub mod remote_desktop;
pub mod screencast;
pub mod screenshot;
/// Retrieve a per-application secret used to encrypt confidential data inside
/// the sandbox.
pub mod secret;
/// Read & listen to system settings changes.
pub mod settings;
pub mod trash;
pub mod usb;
pub mod wallpaper;

#[cfg_attr(feature = "glib", derive(glib::Enum))]
#[cfg_attr(feature = "glib", enum_type(name = "AshpdPersistMode"))]
#[derive(
    Default, serde_repr::Serialize_repr, PartialEq, Eq, Debug, Copy, Clone, zbus::zvariant::Type,
)]
#[doc(alias = "XdpPersistMode")]
#[repr(u32)]
/// Persistence mode for a screencast or remote desktop session.
pub enum PersistMode {
    #[doc(alias = "XDP_PERSIST_MODE_NONE")]
    #[default]
    /// Do not persist.
    DoNot = 0,
    #[doc(alias = "XDP_PERSIST_MODE_TRANSIENT")]
    /// Persist while the application is running.
    Application = 1,
    #[doc(alias = "XDP_PERSIST_MODE_PERSISTENT")]
    /// Persist until explicitly revoked.
    ExplicitlyRevoked = 2,
}
