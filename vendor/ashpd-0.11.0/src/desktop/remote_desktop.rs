//! # Examples
//!
//! ```rust,no_run
//! use ashpd::desktop::{
//!     remote_desktop::{DeviceType, KeyState, RemoteDesktop},
//!     PersistMode,
//! };
//!
//! async fn run() -> ashpd::Result<()> {
//!     let proxy = RemoteDesktop::new().await?;
//!     let session = proxy.create_session().await?;
//!     proxy
//!         .select_devices(
//!             &session,
//!             DeviceType::Keyboard | DeviceType::Pointer,
//!             None,
//!             PersistMode::DoNot,
//!         )
//!         .await?;
//!
//!     let response = proxy.start(&session, None).await?.response()?;
//!     println!("{:#?}", response.devices());
//!
//!     // 13 for Enter key code
//!     proxy
//!         .notify_keyboard_keycode(&session, 13, KeyState::Pressed)
//!         .await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! You can also use the Remote Desktop portal with the ScreenCast one. In order
//! to do so, you need to call
//! [`Screencast::select_sources()`][select_sources]
//! on the session created with
//! [`RemoteDesktop::create_session()`][create_session]
//!
//! ```rust,no_run
//! use ashpd::desktop::{
//!     remote_desktop::{DeviceType, KeyState, RemoteDesktop},
//!     screencast::{CursorMode, Screencast, SourceType},
//!     PersistMode,
//! };
//!
//! async fn run() -> ashpd::Result<()> {
//!     let remote_desktop = RemoteDesktop::new().await?;
//!     let screencast = Screencast::new().await?;
//!     let session = remote_desktop.create_session().await?;
//!
//!     remote_desktop
//!         .select_devices(
//!             &session,
//!             DeviceType::Keyboard | DeviceType::Pointer,
//!             None,
//!             PersistMode::DoNot,
//!         )
//!         .await?;
//!     screencast
//!         .select_sources(
//!             &session,
//!             CursorMode::Metadata,
//!             SourceType::Monitor | SourceType::Window,
//!             true,
//!             None,
//!             PersistMode::DoNot,
//!         )
//!         .await?;
//!
//!     let response = remote_desktop.start(&session, None).await?.response()?;
//!     println!("{:#?}", response.devices());
//!     println!("{:#?}", response.streams());
//!
//!     // 13 for Enter key code
//!     remote_desktop
//!         .notify_keyboard_keycode(&session, 13, KeyState::Pressed)
//!         .await?;
//!
//!     Ok(())
//! }
//! ```
//! [select_sources]: crate::desktop::screencast::Screencast::select_sources
//! [create_session]: crate::desktop::remote_desktop::RemoteDesktop::create_session

use std::{collections::HashMap, os::fd::OwnedFd};

use enumflags2::{bitflags, BitFlags};
use futures_util::TryFutureExt;
use serde_repr::{Deserialize_repr, Serialize_repr};
use zbus::zvariant::{self, DeserializeDict, SerializeDict, Type, Value};

use super::{
    screencast::Stream, session::SessionPortal, HandleToken, PersistMode, Request, Session,
};
use crate::{desktop::session::CreateSessionResponse, proxy::Proxy, Error, WindowIdentifier};

#[cfg_attr(feature = "glib", derive(glib::Enum))]
#[cfg_attr(feature = "glib", enum_type(name = "AshpdKeyState"))]
#[derive(Serialize_repr, Deserialize_repr, Copy, Clone, PartialEq, Eq, Debug, Type)]
#[doc(alias = "XdpKeyState")]
/// The keyboard key state.
#[repr(u32)]
pub enum KeyState {
    #[doc(alias = "XDP_KEY_PRESSED")]
    /// The key is pressed.
    Pressed = 1,
    #[doc(alias = "XDP_KEY_RELEASED")]
    /// The key is released..
    Released = 0,
}

#[bitflags]
#[derive(Serialize_repr, Deserialize_repr, PartialEq, Eq, Debug, Clone, Copy, Type)]
#[repr(u32)]
#[doc(alias = "XdpDeviceType")]
/// A bit flag for the available devices.
pub enum DeviceType {
    #[doc(alias = "XDP_DEVICE_KEYBOARD")]
    /// A keyboard.
    Keyboard,
    #[doc(alias = "XDP_DEVICE_POINTER")]
    /// A mouse pointer.
    Pointer,
    #[doc(alias = "XDP_DEVICE_TOUCHSCREEN")]
    /// A touchscreen
    Touchscreen,
}

#[cfg_attr(feature = "glib", derive(glib::Enum))]
#[cfg_attr(feature = "glib", enum_type(name = "AshpdAxis"))]
#[derive(Serialize_repr, Deserialize_repr, PartialEq, Eq, Debug, Clone, Copy, Type)]
#[doc(alias = "XdpDiscreteAxis")]
#[repr(u32)]
/// The available axis.
pub enum Axis {
    #[doc(alias = "XDP_AXIS_VERTICAL_SCROLL")]
    /// Vertical axis.
    Vertical = 0,
    #[doc(alias = "XDP_AXIS_HORIZONTAL_SCROLL")]
    /// Horizontal axis.
    Horizontal = 1,
}

#[derive(SerializeDict, Type, Debug, Default)]
/// Specified options for a [`RemoteDesktop::create_session`] request.
#[zvariant(signature = "dict")]
struct CreateRemoteOptions {
    /// A string that will be used as the last element of the handle.
    handle_token: HandleToken,
    /// A string that will be used as the last element of the session handle.
    session_handle_token: HandleToken,
}

#[derive(SerializeDict, Type, Debug, Default)]
/// Specified options for a [`RemoteDesktop::select_devices`] request.
#[zvariant(signature = "dict")]
struct SelectDevicesOptions {
    /// A string that will be used as the last element of the handle.
    handle_token: HandleToken,
    /// The device types to request remote controlling of. Default is all.
    types: Option<BitFlags<DeviceType>>,
    restore_token: Option<String>,
    persist_mode: Option<PersistMode>,
}

impl SelectDevicesOptions {
    /// Sets the device types to request remote controlling of.
    pub fn types(mut self, types: impl Into<Option<BitFlags<DeviceType>>>) -> Self {
        self.types = types.into();
        self
    }

    pub fn persist_mode(mut self, persist_mode: impl Into<Option<PersistMode>>) -> Self {
        self.persist_mode = persist_mode.into();
        self
    }

    pub fn restore_token<'a>(mut self, token: impl Into<Option<&'a str>>) -> Self {
        self.restore_token = token.into().map(ToOwned::to_owned);
        self
    }
}

#[derive(SerializeDict, Type, Debug, Default)]
/// Specified options for a [`RemoteDesktop::start`] request.
#[zvariant(signature = "dict")]
struct StartRemoteOptions {
    /// A string that will be used as the last element of the handle.
    handle_token: HandleToken,
}

#[derive(DeserializeDict, Type, Debug, Default)]
/// A response to a [`RemoteDesktop::select_devices`] request.
#[zvariant(signature = "dict")]
pub struct SelectedDevices {
    devices: BitFlags<DeviceType>,
    streams: Option<Vec<Stream>>,
    restore_token: Option<String>,
}

impl SelectedDevices {
    /// The selected devices.
    pub fn devices(&self) -> BitFlags<DeviceType> {
        self.devices
    }

    /// The selected streams if a ScreenCast portal is used on the same session
    pub fn streams(&self) -> Option<&[Stream]> {
        self.streams.as_deref()
    }

    /// The session restore token.
    pub fn restore_token(&self) -> Option<&str> {
        self.restore_token.as_deref()
    }
}

/// The interface lets sandboxed applications create remote desktop sessions.
///
/// Wrapper of the DBus interface: [`org.freedesktop.portal.RemoteDesktop`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html).
#[derive(Debug)]
#[doc(alias = "org.freedesktop.portal.RemoteDesktop")]
pub struct RemoteDesktop<'a>(Proxy<'a>);

impl<'a> RemoteDesktop<'a> {
    /// Create a new instance of [`RemoteDesktop`].
    pub async fn new() -> Result<RemoteDesktop<'a>, Error> {
        let proxy = Proxy::new_desktop("org.freedesktop.portal.RemoteDesktop").await?;
        Ok(Self(proxy))
    }

    /// Create a remote desktop session.
    /// A remote desktop session is used to allow remote controlling a desktop
    /// session. It can also be used together with a screen cast session.
    ///
    /// # Specifications
    ///
    /// See also [`CreateSession`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html#org-freedesktop-portal-remotedesktop-createsession).
    #[doc(alias = "CreateSession")]
    #[doc(alias = "xdp_portal_create_remote_desktop_session")]
    pub async fn create_session(&self) -> Result<Session<'a, Self>, Error> {
        let options = CreateRemoteOptions::default();
        let (request, proxy) = futures_util::try_join!(
            self.0
                .request::<CreateSessionResponse>(&options.handle_token, "CreateSession", &options)
                .into_future(),
            Session::from_unique_name(&options.session_handle_token).into_future()
        )?;
        assert_eq!(proxy.path(), &request.response()?.session_handle.as_ref());
        Ok(proxy)
    }

    /// Select input devices to remote control.
    ///
    /// # Arguments
    ///
    /// * `session` - A [`Session`], created with
    ///   [`create_session()`][`RemoteDesktop::create_session`].
    /// * `types` - The device types to request remote controlling of.
    ///
    /// # Specifications
    ///
    /// See also [`SelectDevices`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html#org-freedesktop-portal-remotedesktop-selectdevices).
    #[doc(alias = "SelectDevices")]
    pub async fn select_devices(
        &self,
        session: &Session<'_, Self>,
        types: BitFlags<DeviceType>,
        restore_token: Option<&str>,
        persist_mode: PersistMode,
    ) -> Result<Request<()>, Error> {
        let options = SelectDevicesOptions::default()
            .types(types)
            .persist_mode(persist_mode)
            .restore_token(restore_token);
        self.0
            .empty_request(&options.handle_token, "SelectDevices", &(session, &options))
            .await
    }

    ///  Start the remote desktop session.
    ///
    /// This will typically result in the portal presenting a dialog letting
    /// the user select what to share, including devices and optionally screen
    /// content if screen cast sources was selected.
    ///
    /// # Arguments
    ///
    /// * `session` - A [`Session`], created with
    ///   [`create_session()`][`RemoteDesktop::create_session`].
    /// * `identifier` - The application window identifier.
    ///
    /// # Specifications
    ///
    /// See also [`Start`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html#org-freedesktop-portal-remotedesktop-start).
    #[doc(alias = "Start")]
    pub async fn start(
        &self,
        session: &Session<'_, Self>,
        identifier: Option<&WindowIdentifier>,
    ) -> Result<Request<SelectedDevices>, Error> {
        let options = StartRemoteOptions::default();
        let identifier = identifier.map(|i| i.to_string()).unwrap_or_default();
        self.0
            .request(
                &options.handle_token,
                "Start",
                &(session, &identifier, &options),
            )
            .await
    }

    /// Notify keyboard code.
    ///
    /// **Note** only works if [`DeviceType::Keyboard`] access was provided
    /// after starting the session.
    ///
    /// # Arguments
    ///
    /// * `session` - A [`Session`], created with
    ///   [`create_session()`][`RemoteDesktop::create_session`].
    /// * `keycode` - Keyboard code that was pressed or released.
    /// * `state` - The new state of the keyboard code.
    ///
    /// # Specifications
    ///
    /// See also [`NotifyKeyboardKeycode`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html#org-freedesktop-portal-remotedesktop-notifykeyboardkeycode).
    #[doc(alias = "NotifyKeyboardKeycode")]
    pub async fn notify_keyboard_keycode(
        &self,
        session: &Session<'_, Self>,
        keycode: i32,
        state: KeyState,
    ) -> Result<(), Error> {
        // The `notify` methods don't take any options for now
        // see https://github.com/flatpak/xdg-desktop-portal/blob/master/src/remote-desktop.c#L723
        let options: HashMap<&str, Value<'_>> = HashMap::new();
        self.0
            .call("NotifyKeyboardKeycode", &(session, options, keycode, state))
            .await
    }

    /// Notify keyboard symbol.
    ///
    /// **Note** only works if [`DeviceType::Keyboard`] access was provided
    /// after starting the session.
    ///
    /// # Arguments
    ///
    /// * `session` - A [`Session`], created with
    ///   [`create_session()`][`RemoteDesktop::create_session`].
    /// * `keysym` - Keyboard symbol that was pressed or released.
    /// * `state` - The new state of the keyboard code.
    ///
    /// # Specifications
    ///
    /// See also [`NotifyKeyboardKeysym`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html#org-freedesktop-portal-remotedesktop-notifykeyboardkeysym).
    #[doc(alias = "NotifyKeyboardKeysym")]
    pub async fn notify_keyboard_keysym(
        &self,
        session: &Session<'_, Self>,
        keysym: i32,
        state: KeyState,
    ) -> Result<(), Error> {
        // The `notify` methods don't take any options for now
        // see https://github.com/flatpak/xdg-desktop-portal/blob/master/src/remote-desktop.c#L723
        let options: HashMap<&str, Value<'_>> = HashMap::new();
        self.0
            .call("NotifyKeyboardKeysym", &(session, options, keysym, state))
            .await
    }

    /// Notify about a new touch up event.
    ///
    /// **Note** only works if [`DeviceType::Touchscreen`] access was provided
    /// after starting the session.
    ///
    /// # Arguments
    ///
    /// * `session` - A [`Session`], created with
    ///   [`create_session()`][`RemoteDesktop::create_session`].
    /// * `slot` - Touch slot where touch point appeared.
    ///
    /// # Specifications
    ///
    /// See also [`NotifyTouchUp`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html#org-freedesktop-portal-remotedesktop-notifytouchup).
    #[doc(alias = "NotifyTouchUp")]
    pub async fn notify_touch_up(
        &self,
        session: &Session<'_, Self>,
        slot: u32,
    ) -> Result<(), Error> {
        // The `notify` methods don't take any options for now
        // see https://github.com/flatpak/xdg-desktop-portal/blob/master/src/remote-desktop.c#L723
        let options: HashMap<&str, Value<'_>> = HashMap::new();
        self.0
            .call("NotifyTouchUp", &(session, options, slot))
            .await
    }

    /// Notify about a new touch down event.
    /// The (x, y) position represents the new touch point position in the
    /// streams logical coordinate space.
    ///
    /// **Note** only works if [`DeviceType::Touchscreen`] access was provided
    /// after starting the session.
    ///
    /// # Arguments
    ///
    /// * `session` - A [`Session`], created with
    ///   [`create_session()`][`RemoteDesktop::create_session`].
    /// * `stream` - The PipeWire stream node the coordinate is relative to.
    /// * `slot` - Touch slot where touch point appeared.
    /// * `x` - Touch down x coordinate.
    /// * `y` - Touch down y coordinate.
    ///
    /// # Specifications
    ///
    /// See also [`NotifyTouchDown`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html#org-freedesktop-portal-remotedesktop-notifytouchdown).
    #[doc(alias = "NotifyTouchDown")]
    pub async fn notify_touch_down(
        &self,
        session: &Session<'_, Self>,
        stream: u32,
        slot: u32,
        x: f64,
        y: f64,
    ) -> Result<(), Error> {
        // The `notify` methods don't take any options for now
        // see https://github.com/flatpak/xdg-desktop-portal/blob/master/src/remote-desktop.c#L723
        let options: HashMap<&str, Value<'_>> = HashMap::new();
        self.0
            .call("NotifyTouchDown", &(session, options, stream, slot, x, y))
            .await
    }

    /// Notify about a new touch motion event.
    /// The (x, y) position represents where the touch point position in the
    /// streams logical coordinate space moved.
    ///
    /// **Note** only works if [`DeviceType::Touchscreen`] access was provided
    /// after starting the session.
    ///
    /// # Arguments
    ///
    /// * `session` - A [`Session`], created with
    ///   [`create_session()`][`RemoteDesktop::create_session`].
    /// * `stream` - The PipeWire stream node the coordinate is relative to.
    /// * `slot` - Touch slot where touch point appeared.
    /// * `x` - Touch motion x coordinate.
    /// * `y` - Touch motion y coordinate.
    ///
    /// # Specifications
    ///
    /// See also [`NotifyTouchMotion`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html#org-freedesktop-portal-remotedesktop-notifytouchmotion).
    #[doc(alias = "NotifyTouchMotion")]
    pub async fn notify_touch_motion(
        &self,
        session: &Session<'_, Self>,
        stream: u32,
        slot: u32,
        x: f64,
        y: f64,
    ) -> Result<(), Error> {
        // The `notify` methods don't take any options for now
        // see https://github.com/flatpak/xdg-desktop-portal/blob/master/src/remote-desktop.c#L723
        let options: HashMap<&str, Value<'_>> = HashMap::new();
        self.0
            .call("NotifyTouchMotion", &(session, options, stream, slot, x, y))
            .await
    }

    /// Notify about a new absolute pointer motion event.
    /// The (x, y) position represents the new pointer position in the streams
    /// logical coordinate space.
    ///
    /// # Arguments
    ///
    /// * `session` - A [`Session`], created with
    ///   [`create_session()`][`RemoteDesktop::create_session`].
    /// * `stream` - The PipeWire stream node the coordinate is relative to.
    /// * `x` - Pointer motion x coordinate.
    /// * `y` - Pointer motion y coordinate.
    ///
    /// # Specifications
    ///
    /// See also [`NotifyPointerMotionAbsolute`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html#org-freedesktop-portal-remotedesktop-notifypointermotionabsolute).
    #[doc(alias = "NotifyPointerMotionAbsolute")]
    pub async fn notify_pointer_motion_absolute(
        &self,
        session: &Session<'_, Self>,
        stream: u32,
        x: f64,
        y: f64,
    ) -> Result<(), Error> {
        // The `notify` methods don't take any options for now
        // see https://github.com/flatpak/xdg-desktop-portal/blob/master/src/remote-desktop.c#L723
        let options: HashMap<&str, Value<'_>> = HashMap::new();
        self.0
            .call(
                "NotifyPointerMotionAbsolute",
                &(session, options, stream, x, y),
            )
            .await
    }

    /// Notify about a new relative pointer motion event.
    /// The (dx, dy) vector represents the new pointer position in the streams
    /// logical coordinate space.
    ///
    /// # Arguments
    ///
    /// * `session` - A [`Session`], created with
    ///   [`create_session()`][`RemoteDesktop::create_session`].
    /// * `dx` - Relative movement on the x axis.
    /// * `dy` - Relative movement on the y axis.
    ///
    /// # Specifications
    ///
    /// See also [`NotifyPointerMotion`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html#org-freedesktop-portal-remotedesktop-notifypointermotion).
    #[doc(alias = "NotifyPointerMotion")]
    pub async fn notify_pointer_motion(
        &self,
        session: &Session<'_, Self>,
        dx: f64,
        dy: f64,
    ) -> Result<(), Error> {
        // The `notify` methods don't take any options for now
        // see https://github.com/flatpak/xdg-desktop-portal/blob/master/src/remote-desktop.c#L723
        let options: HashMap<&str, Value<'_>> = HashMap::new();
        self.0
            .call("NotifyPointerMotion", &(session, options, dx, dy))
            .await
    }

    /// Notify pointer button.
    /// The pointer button is encoded according to Linux Evdev button codes.
    ///
    ///
    /// **Note** only works if [`DeviceType::Pointer`] access was provided after
    /// starting the session.
    ///
    /// # Arguments
    ///
    /// * `session` - A [`Session`], created with
    ///   [`create_session()`][`RemoteDesktop::create_session`].
    /// * `button` - The pointer button was pressed or released.
    /// * `state` - The new state of the keyboard code.
    ///
    /// # Specifications
    ///
    /// See also [`NotifyPointerButton`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html#org-freedesktop-portal-remotedesktop-notifypointerbutton).
    #[doc(alias = "NotifyPointerButton")]
    pub async fn notify_pointer_button(
        &self,
        session: &Session<'_, Self>,
        button: i32,
        state: KeyState,
    ) -> Result<(), Error> {
        // The `notify` methods don't take any options for now
        // see https://github.com/flatpak/xdg-desktop-portal/blob/master/src/remote-desktop.c#L723
        let options: HashMap<&str, Value<'_>> = HashMap::new();
        self.0
            .call("NotifyPointerButton", &(session, options, button, state))
            .await
    }

    /// Notify pointer axis discrete.
    ///
    /// **Note** only works if [`DeviceType::Pointer`] access was provided after
    /// starting the session.
    ///
    /// # Arguments
    ///
    /// * `session` - A [`Session`], created with
    ///   [`create_session()`][`RemoteDesktop::create_session`].
    /// * `axis` - The axis that was scrolled.
    ///
    /// # Specifications
    ///
    /// See also [`NotifyPointerAxisDiscrete`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html#org-freedesktop-portal-remotedesktop-notifypointeraxisdiscrete).
    #[doc(alias = "NotifyPointerAxisDiscrete")]
    pub async fn notify_pointer_axis_discrete(
        &self,
        session: &Session<'_, Self>,
        axis: Axis,
        steps: i32,
    ) -> Result<(), Error> {
        // The `notify` methods don't take any options for now
        // see https://github.com/flatpak/xdg-desktop-portal/blob/master/src/remote-desktop.c#L723
        let options: HashMap<&str, Value<'_>> = HashMap::new();
        self.0
            .call(
                "NotifyPointerAxisDiscrete",
                &(session, options, axis, steps),
            )
            .await
    }

    /// Notify pointer axis.
    /// The axis movement from a "smooth scroll" device, such as a touchpad.
    /// When applicable, the size of the motion delta should be equivalent to
    /// the motion vector of a pointer motion done using the same advice.
    ///
    ///
    /// **Note** only works if [`DeviceType::Pointer`] access was provided after
    /// starting the session.
    ///
    /// # Arguments
    ///
    /// * `session` - A [`Session`], created with
    ///   [`create_session()`][`RemoteDesktop::create_session`].
    /// * `dx` - Relative axis movement on the x axis.
    /// * `dy` - Relative axis movement on the y axis.
    /// * `finish` - Whether it is the last axis event.
    ///
    /// # Specifications
    ///
    /// See also [`NotifyPointerAxis`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html#org-freedesktop-portal-remotedesktop-notifypointeraxis).
    #[doc(alias = "NotifyPointerAxis")]
    pub async fn notify_pointer_axis(
        &self,
        session: &Session<'_, Self>,
        dx: f64,
        dy: f64,
        finish: bool,
    ) -> Result<(), Error> {
        // see https://github.com/flatpak/xdg-desktop-portal/blob/master/src/remote-desktop.c#L911
        let mut options: HashMap<&str, Value<'_>> = HashMap::new();
        options.insert("finish", Value::Bool(finish));
        self.0
            .call("NotifyPointerAxis", &(session, options, dx, dy))
            .await
    }

    /// Connect to EIS.
    ///
    /// **Note** only succeeds if called after [`RemoteDesktop::start`].
    ///
    /// Requires RemoteDesktop version 2.
    ///
    /// # Arguments
    ///
    /// * `session` - A [`Session`], created with
    ///   [`create_session()`][`RemoteDesktop::create_session`].
    ///
    /// # Required version
    ///
    /// The method requires the 2nd version implementation of the portal and
    /// would fail with [`Error::RequiresVersion`] otherwise.
    ///
    /// # Specifications
    ///
    /// See also [`ConnectToEIS`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html#org-freedesktop-portal-remotedesktop-connecttoeis).
    #[doc(alias = "ConnectToEIS")]
    pub async fn connect_to_eis(&self, session: &Session<'_, Self>) -> Result<OwnedFd, Error> {
        // `ConnectToEIS` doesn't take any options for now
        // see https://github.com/flatpak/xdg-desktop-portal/blob/master/src/remote-desktop.c#L1464
        let options: HashMap<&str, Value<'_>> = HashMap::new();
        let fd = self
            .0
            .call_versioned::<zvariant::OwnedFd>("ConnectToEIS", &(session, options), 2)
            .await?;
        Ok(fd.into())
    }

    /// Available source types.
    ///
    /// # Specifications
    ///
    /// See also [`AvailableDeviceTypes`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html#org-freedesktop-portal-remotedesktop-availabledevicetypes).
    #[doc(alias = "AvailableDeviceTypes")]
    pub async fn available_device_types(&self) -> Result<BitFlags<DeviceType>, Error> {
        self.0.property("AvailableDeviceTypes").await
    }
}

impl<'a> std::ops::Deref for RemoteDesktop<'a> {
    type Target = zbus::Proxy<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl crate::Sealed for RemoteDesktop<'_> {}
impl SessionPortal for RemoteDesktop<'_> {}
