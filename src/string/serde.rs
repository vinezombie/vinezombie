use super::Bytes;

#[derive(Clone, Copy, Default, Debug)]
struct Visitor<'a>(std::marker::PhantomData<&'a [u8]>);

impl<'a> serde::Serialize for Bytes<'a> {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(string) = self.to_utf8() {
            ser.serialize_str(string)
        } else {
            ser.serialize_bytes(self.as_bytes())
        }
    }
}

impl<'a, 'de> serde::Deserialize<'de> for Bytes<'a> {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        de.deserialize_string(Visitor::<'a>(std::marker::PhantomData))
    }
}

impl<'a, 'de> serde::de::Visitor<'de> for Visitor<'a> {
    type Value = Bytes<'a>;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "a string or byte array (either owning or borrowed) or false/unit")
    }

    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if !v {
            Ok(Bytes::empty())
        } else {
            Err(serde::de::Error::invalid_type(serde::de::Unexpected::Bool(v), &self))
        }
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Bytes::from_str(v).owning())
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(v.into())
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Bytes::from_bytes(v).owning())
    }

    fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(v.into())
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Bytes::empty())
    }
}
