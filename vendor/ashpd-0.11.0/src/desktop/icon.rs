use std::os::fd::AsFd;

use serde::{
    de,
    ser::{Serialize, SerializeTuple},
    Deserialize,
};
use zbus::zvariant::{self, OwnedValue, Type, Value};

use crate::Error;

#[derive(Debug, Type)]
#[zvariant(signature = "(sv)")]
/// A representation of an icon.
///
/// Used by both the Notification & Dynamic launcher portals.
pub enum Icon {
    /// An icon URI.
    Uri(url::Url),
    /// A list of icon names.
    Names(Vec<String>),
    /// Icon bytes.
    Bytes(Vec<u8>),
    /// A file descriptor.
    FileDescriptor(std::os::fd::OwnedFd),
}

impl Icon {
    /// Create an icon from a list of names.
    pub fn with_names<N>(names: impl IntoIterator<Item = N>) -> Self
    where
        N: ToString,
    {
        Self::Names(names.into_iter().map(|name| name.to_string()).collect())
    }

    pub(crate) fn is_bytes(&self) -> bool {
        matches!(self, Self::Bytes(_))
    }

    pub(crate) fn inner_bytes(&self) -> Value {
        match self {
            Self::Bytes(bytes) => {
                let mut array = zvariant::Array::new(u8::SIGNATURE);
                for byte in bytes.iter() {
                    // Safe to unwrap because we are sure it is of the correct type
                    array.append(Value::from(*byte)).unwrap();
                }
                Value::from(array)
            }
            _ => panic!("Only bytes icons can be converted to a bytes variant"),
        }
    }

    pub(crate) fn as_value(&self) -> Value {
        let tuple = match self {
            Self::Uri(uri) => ("file", Value::from(uri.as_str())),
            Self::Names(names) => {
                let mut array = zvariant::Array::new(String::SIGNATURE);
                for name in names.iter() {
                    // Safe to unwrap because we are sure it is of the correct type
                    array.append(Value::from(name)).unwrap();
                }

                ("themed", Value::from(array))
            }
            Self::Bytes(_) => ("bytes", self.inner_bytes()),
            Self::FileDescriptor(fd) => ("file-descriptor", zvariant::Fd::from(fd).into()),
        };
        Value::new(tuple)
    }
}

impl Serialize for Icon {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut tuple = serializer.serialize_tuple(2)?;
        match self {
            Self::Uri(uri) => {
                tuple.serialize_element("file")?;
                tuple.serialize_element(&Value::from(uri.as_str()))?;
            }
            Self::Names(names) => {
                tuple.serialize_element("themed")?;
                let mut array = zvariant::Array::new(String::SIGNATURE);
                for name in names.iter() {
                    // Safe to unwrap because we are sure it is of the correct type
                    array.append(Value::from(name)).unwrap();
                }
                tuple.serialize_element(&Value::from(array))?;
            }
            Self::Bytes(_) => {
                tuple.serialize_element("bytes")?;
                tuple.serialize_element(&self.inner_bytes())?;
            }
            Self::FileDescriptor(fd) => {
                tuple.serialize_element("file-descriptor")?;
                tuple.serialize_element(&Value::from(zvariant::Fd::from(fd)))?;
            }
        }
        tuple.end()
    }
}

impl<'de> Deserialize<'de> for Icon {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let (type_, data) = <(String, OwnedValue)>::deserialize(deserializer)?;
        match type_.as_str() {
            "file" => {
                let uri_str = data.downcast_ref::<zvariant::Str>().unwrap();
                let uri = url::Url::parse(uri_str.as_str())
                    .map_err(|_| de::Error::custom("Couldn't deserialize Icon of type 'file'"))?;
                Ok(Self::Uri(uri))
            }
            "bytes" => {
                let array = data.downcast_ref::<zvariant::Array>().unwrap();
                let mut bytes = Vec::with_capacity(array.len());
                for byte in array.inner() {
                    bytes.push(byte.downcast_ref::<u8>().unwrap());
                }
                Ok(Self::Bytes(bytes))
            }
            "themed" => {
                let array = data.downcast_ref::<zvariant::Array>().unwrap();
                let mut names = Vec::with_capacity(array.len());
                for value in array.inner() {
                    let name = value.downcast_ref::<zvariant::Str>().unwrap();
                    names.push(name.as_str().to_owned());
                }
                Ok(Self::Names(names))
            }
            "file-descriptor" => {
                let fd = data.downcast_ref::<zvariant::Fd>().unwrap();
                Ok(Self::FileDescriptor(
                    fd.as_fd()
                        .try_clone_to_owned()
                        .expect("Failed to clone file descriptor"),
                ))
            }
            _ => Err(de::Error::custom("Invalid Icon type")),
        }
    }
}

impl TryFrom<&OwnedValue> for Icon {
    type Error = crate::Error;
    fn try_from(value: &OwnedValue) -> Result<Self, Self::Error> {
        let structure = value.downcast_ref::<zvariant::Structure>().unwrap();
        let fields = structure.fields();
        let type_ = fields[0].downcast_ref::<zvariant::Str>().unwrap();
        match type_.as_str() {
            "file" => {
                let uri_str = fields[1]
                    .downcast_ref::<zvariant::Str>()
                    .unwrap()
                    .to_owned();
                let uri = url::Url::parse(uri_str.as_str())
                    .map_err(|_| crate::Error::ParseError("Failed to parse uri"))?;
                Ok(Self::Uri(uri))
            }
            "bytes" => {
                let array = fields[1].downcast_ref::<zvariant::Array>().unwrap();
                let mut bytes = Vec::with_capacity(array.len());
                for byte in array.inner() {
                    bytes.push(byte.downcast_ref::<u8>().unwrap());
                }
                Ok(Self::Bytes(bytes))
            }
            "themed" => {
                let array = fields[1].downcast_ref::<zvariant::Array>().unwrap();
                let mut names = Vec::with_capacity(array.len());
                for value in array.inner() {
                    let name = value.downcast_ref::<zvariant::Str>().unwrap();
                    names.push(name.as_str().to_owned());
                }
                Ok(Self::Names(names))
            }
            "file-descriptor" => {
                let fd = fields[1].downcast_ref::<zvariant::Fd>().unwrap();
                Ok(Self::FileDescriptor(
                    fd.as_fd()
                        .try_clone_to_owned()
                        .expect("Failed to clone file descriptor"),
                ))
            }
            _ => Err(Error::ParseError("Invalid Icon type")),
        }
    }
}

impl TryFrom<OwnedValue> for Icon {
    type Error = crate::Error;
    fn try_from(value: OwnedValue) -> Result<Self, Self::Error> {
        Self::try_from(&value)
    }
}

impl TryFrom<Value<'_>> for Icon {
    type Error = crate::Error;
    fn try_from(value: Value<'_>) -> Result<Self, Self::Error> {
        Self::try_from(&value)
    }
}
impl TryFrom<&Value<'_>> for Icon {
    type Error = crate::Error;
    fn try_from(value: &Value<'_>) -> Result<Self, Self::Error> {
        Self::try_from(value.try_to_owned()?)
    }
}

#[cfg(test)]
mod test {
    use zbus::zvariant::{serialized::Context, to_bytes, Endian};

    use super::*;

    #[test]
    fn check_icon_signature() {
        assert_eq!(Icon::SIGNATURE, "(sv)");
    }

    #[test]
    fn serialize_deserialize() {
        let ctxt = Context::new_dbus(Endian::Little, 0);

        let icon = Icon::with_names(["dialog-symbolic"]);

        let encoded = to_bytes(ctxt, &icon).unwrap();
        let decoded: Icon = encoded.deserialize().unwrap().0;
        assert!(matches!(decoded, Icon::Names(_)));

        let icon = Icon::Uri(url::Url::parse("file://some/icon.png").unwrap());
        let encoded = to_bytes(ctxt, &icon).unwrap();
        let decoded: Icon = encoded.deserialize().unwrap().0;
        assert!(matches!(decoded, Icon::Uri(_)));

        let icon = Icon::Bytes(vec![1, 0, 1, 0]);
        let encoded = to_bytes(ctxt, &icon).unwrap();
        let decoded: Icon = encoded.deserialize().unwrap().0;
        assert!(matches!(decoded, Icon::Bytes(_)));

        let fd = std::fs::File::open("/tmp").unwrap();
        let icon = Icon::FileDescriptor(fd.as_fd().try_clone_to_owned().unwrap());
        let encoded = to_bytes(ctxt, &icon).unwrap();
        let decoded: Icon = encoded.deserialize().unwrap().0;
        assert!(matches!(decoded, Icon::FileDescriptor(_)));
    }
}
