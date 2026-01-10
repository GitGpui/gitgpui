// Copyright (C) 2024-2025 GNOME Foundation
//
// Authors:
//     Hubert Figuière <hub@figuiere.net>
//

//! Provide an interface to USB device. Allow enumerating devices
//! and requiring access to.
//! ```rust,no_run
//! use ashpd::desktop::usb::UsbProxy;
//! use futures_util::StreamExt;
//!
//! async fn watch_devices() -> ashpd::Result<()> {
//!     let usb = UsbProxy::new().await?;
//!     let session = usb.create_session().await?;
//!     if let Some(response) = usb.receive_device_events().await?.next().await {
//!         let events = response.events();
//!         for ev in events {
//!             println!(
//!                 "Received event: {} for device {}",
//!                 ev.action(),
//!                 ev.device_id()
//!             );
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```

use std::collections::HashMap;

use futures_util::Stream;
use serde::Deserialize;
use zbus::zvariant::{
    DeserializeDict, ObjectPath, OwnedFd, OwnedObjectPath, OwnedValue, SerializeDict, Type, Value,
};

use crate::{
    desktop::{HandleToken, Session, SessionPortal},
    proxy::Proxy,
    Error, WindowIdentifier,
};

#[derive(Debug, SerializeDict, Type, Default)]
#[zvariant(signature = "dict")]
struct CreateSessionOptions {
    handle_token: HandleToken,
    session_handle_token: HandleToken,
}

/// Options for the USB portal.
#[derive(SerializeDict, Type, Debug, Default)]
#[zvariant(signature = "dict")]
struct UsbEnumerateOptions {}

/// USB device description
#[derive(SerializeDict, DeserializeDict, Type, Debug, Default)]
#[zvariant(signature = "dict")]
pub struct UsbDevice {
    parent: Option<String>,
    /// Device can be opened for reading. Default is false.
    readable: Option<bool>,
    /// Device can be opened for writing. Default is false.
    writable: Option<bool>,
    /// The device node for the USB.
    #[zvariant(rename = "device-file")]
    device_file: Option<String>,
    /// Device properties
    properties: Option<HashMap<String, OwnedValue>>,
}

impl UsbDevice {
    /// Device ID of the parent device
    pub fn parent(&self) -> Option<&str> {
        self.parent.as_deref()
    }

    /// Return if the device is readable.
    pub fn is_readable(&self) -> bool {
        self.readable.unwrap_or(false)
    }

    /// Return if the device is writable.
    pub fn is_writable(&self) -> bool {
        self.writable.unwrap_or(false)
    }

    /// Return the optional device file.
    pub fn device_file(&self) -> Option<&str> {
        self.device_file.as_deref()
    }

    /// Return the vendor string property for display.
    pub fn vendor(&self) -> Option<String> {
        self.properties.as_ref().and_then(|properties| {
            properties
                .get("ID_VENDOR_FROM_DATABASE")
                .or_else(|| properties.get("ID_VENDOR_ENC"))
                .and_then(|v| v.downcast_ref::<String>().ok())
        })
    }

    /// Return the model string property for display.
    pub fn model(&self) -> Option<String> {
        self.properties.as_ref().and_then(|properties| {
            properties
                .get("ID_MODEL_FROM_DATABASE")
                .or_else(|| properties.get("ID_MODEL_ENC"))
                .and_then(|v| v.downcast_ref::<String>().ok())
        })
    }

    /// Return the device properties.
    pub fn properties(&self) -> Option<&HashMap<String, OwnedValue>> {
        self.properties.as_ref()
    }
}

/// USB error for acquiring device.
#[derive(Debug)]
pub struct UsbError(pub Option<String>);

impl std::fmt::Display for UsbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.as_deref().unwrap_or(""))
    }
}

impl std::error::Error for UsbError {}

impl From<AcquiredDevice> for Result<OwnedFd, UsbError> {
    fn from(v: AcquiredDevice) -> Result<OwnedFd, UsbError> {
        if let Some(fd) = v.fd {
            if v.success {
                Ok(fd)
            } else {
                Err(UsbError(v.error))
            }
        } else {
            Err(UsbError(None))
        }
    }
}

/// Device to acquire.
#[derive(SerializeDict, Type, Debug, Default)]
#[zvariant(signature = "dict")]
struct AcquireDevice {
    writable: bool,
}

/// Device to acquire. Tuple with the device ID and whether the
/// requested permission should be to write.
pub struct Device(String /* ID */, bool /* writable */);

impl Device {
    /// Create a new `Device`.
    pub fn new(id: String, writable: bool) -> Device {
        Device(id, writable)
    }

    /// Get the device ID.
    pub fn id(&self) -> &str {
        &self.0
    }

    /// Return whether the device is writable.
    pub fn is_writable(&self) -> bool {
        self.1
    }
}

/// Option for AcquireDevice call.
#[derive(SerializeDict, Type, Debug, Default)]
#[zvariant(signature = "dict")]
struct AcquireOptions {
    handle_token: HandleToken,
}

/// Finished device acquired.
#[derive(DeserializeDict, Type, Debug, Default)]
#[zvariant(signature = "dict")]
struct AcquiredDevice {
    success: bool,
    fd: Option<OwnedFd>,
    error: Option<String>,
}

#[derive(Debug, Deserialize, Type)]
/// A USB event received part of the `device_event` signal response.
/// An event is composed of an `action`, a device `id` and the
/// [`UsbDevice`] description.
pub struct UsbEvent(/* action */ String, /* id */ String, UsbDevice);

impl UsbEvent {
    /// The action. Possible values are `"add"`, `"change"` or `"remove"`.
    pub fn action(&self) -> &str {
        &self.0
    }

    /// The device ID string.
    pub fn device_id(&self) -> &str {
        &self.1
    }

    /// The [`UsbDevice`] properties.
    pub fn device(&self) -> &UsbDevice {
        &self.2
    }
}

#[derive(Debug, Deserialize, Type)]
/// A response received when the `device_event` signal is received.
pub struct UsbDeviceEvent(OwnedObjectPath, Vec<UsbEvent>);

impl UsbDeviceEvent {
    /// The session that triggered the state change
    pub fn session_handle(&self) -> ObjectPath<'_> {
        self.0.as_ref()
    }

    /// Events received
    pub fn events(&self) -> &[UsbEvent] {
        &self.1
    }
}

/// This interface provides access to USB devices.
#[derive(Debug)]
#[doc(alias = "org.freedesktop.portal.Usb")]
pub struct UsbProxy<'a>(Proxy<'a>);

impl<'a> UsbProxy<'a> {
    /// Create a new instance of [`UsbProxy`].
    pub async fn new() -> Result<UsbProxy<'a>, Error> {
        let proxy = Proxy::new_desktop("org.freedesktop.portal.Usb").await?;
        Ok(Self(proxy))
    }

    /// Create a USB session.
    ///
    /// While this session is active, the caller will receive
    /// `DeviceEvents` signals with device addition and removal.
    ///
    /// # Specifications
    ///
    /// See also [`CreateSession`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Usb.html#org-freedesktop-portal-usb-createsession).
    #[doc(alias = "CreateSession")]
    pub async fn create_session(&self) -> Result<Session<'a, Self>, Error> {
        let options = CreateSessionOptions::default();
        let session: OwnedObjectPath = self.0.call("CreateSession", &(&options)).await?;
        Session::new(session).await
    }

    /// Enumerate USB devices.
    ///
    /// Return a vector of tuples with the device ID and the [`UsbDevice`]
    ///
    /// # Specifications
    ///
    /// See also [`EnumerateDevices`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Usb.html#org-freedesktop-portal-usb-enumeratedevices).
    #[doc(alias = "EnumerateDevices")]
    pub async fn enumerate_devices(&self) -> Result<Vec<(String, UsbDevice)>, Error> {
        let options = UsbEnumerateOptions::default();
        self.0.call("EnumerateDevices", &(&options)).await
    }

    /// Acquire devices
    ///
    /// The portal will perform the permission request. In case of
    /// success, ie permission is granted, it returns a vector of
    /// tuples containing the device ID and the file descriptor or
    /// error.
    ///
    /// # Specifications
    ///
    /// See also [`AcquireDevices`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Usb.html#org-freedesktop-portal-usb-acquiredevices).
    #[doc(alias = "AcquireDevices")]
    pub async fn acquire_devices(
        &self,
        parent_window: Option<&WindowIdentifier>,
        devices: &[Device],
    ) -> Result<Vec<(String, Result<OwnedFd, UsbError>)>, Error> {
        let options = AcquireOptions::default();
        let parent_window = parent_window.map(|i| i.to_string()).unwrap_or_default();
        let acquire_devices: Vec<(String, AcquireDevice)> = devices
            .iter()
            .map(|dev| {
                let device = AcquireDevice { writable: dev.1 };
                (dev.0.to_string(), device)
            })
            .collect();
        let request = self
            .0
            .empty_request(
                &options.handle_token,
                "AcquireDevices",
                &(&parent_window, &acquire_devices, &options),
            )
            .await?;
        let mut devices: Vec<(String, Result<OwnedFd, UsbError>)> = vec![];
        if request.response().is_ok() {
            let path = request.path();
            loop {
                let (mut new_devices, finished) = self.finish_acquire_devices(path).await?;
                devices.append(&mut new_devices);
                if finished {
                    break;
                }
            }
        }
        Ok(devices)
    }

    /// Call on success of acquire_devices. This never need to be called
    /// by client applications.
    ///
    /// # Specifications
    ///
    /// See also [`FinishAcquireDevices`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Usb.html#org-freedesktop-portal-usb-finishacquiredevices).
    #[doc(alias = "FinishAcquireDevices")]
    async fn finish_acquire_devices(
        &self,
        request_path: &ObjectPath<'_>,
    ) -> Result<(Vec<(String, Result<OwnedFd, UsbError>)>, bool), Error> {
        let options: HashMap<&str, Value<'_>> = HashMap::new();
        self.0
            .call("FinishAcquireDevices", &(request_path, &options))
            .await
            .map(|result: (Vec<(String, AcquiredDevice)>, bool)| {
                let finished = result.1;
                (
                    result
                        .0
                        .into_iter()
                        .map(|item| (item.0, item.1.into()))
                        .collect::<Vec<_>>(),
                    finished,
                )
            })
    }

    /// Release devices
    ///
    /// Release all the devices whose ID is specified in `devices`.
    ///
    /// # Specifications
    ///
    /// See also [`ReleaseDevices`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Usb.html#org-freedesktop-portal-usb-releasedevices).
    #[doc(alias = "ReleaseDevices")]
    pub async fn release_devices(&self, devices: &[&str]) -> Result<(), Error> {
        let options: HashMap<&str, Value<'_>> = HashMap::new();
        self.0.call("ReleaseDevices", &(devices, &options)).await
    }

    /// Signal emitted on a device event
    ///
    /// Will emit [`UsbDeviceEvent`].
    ///
    /// # Specifications
    ///
    /// See also [`DeviceEvents`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Usb.html#org-freedesktop-portal-usb-deviceevents).
    #[doc(alias = "DeviceEvents")]
    pub async fn receive_device_events(&self) -> Result<impl Stream<Item = UsbDeviceEvent>, Error> {
        self.0.signal("DeviceEvents").await
    }
}

impl crate::Sealed for UsbProxy<'_> {}
impl SessionPortal for UsbProxy<'_> {}
