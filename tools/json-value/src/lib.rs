use std::fmt::{self, Formatter};

use serde::{
    de::{self, value::SeqAccessDeserializer, Deserialize, Deserializer, MapAccess, SeqAccess},
    ser::{Serialize, Serializer},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    I64(i64),
    U64(u64),
    String(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

impl Serialize for JsonValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *self {
            JsonValue::Null => serializer.serialize_none(),
            JsonValue::Bool(value) => serializer.serialize_bool(value),
            JsonValue::I64(value) => serializer.serialize_i64(value),
            JsonValue::U64(value) => serializer.serialize_u64(value),
            JsonValue::String(ref value) => serializer.serialize_str(&value),
            JsonValue::Array(ref value) => serializer.collect_seq(value),
            JsonValue::Object(ref value) => {
                serializer.collect_map(value.iter().map(|(k, v)| (k, v)))
            }
        }
    }
}

impl<'de> Deserialize<'de> for JsonValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(DeVisitor {})
    }
}

struct DeVisitor;

impl<'de> de::Visitor<'de> for DeVisitor {
    type Value = JsonValue;

    fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
        formatter.write_str("any valid JSON value, except floating point values")
    }

    #[inline]
    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Self::Value::Bool(v))
    }

    #[inline]
    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Self::Value::I64(v))
    }

    #[inline]
    fn visit_i128<E>(self, v: i128) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        v.try_into().map_err(E::custom).map(JsonValue::I64)
    }

    #[inline]
    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Self::Value::U64(v))
    }

    #[inline]
    fn visit_u128<E>(self, v: u128) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        v.try_into().map_err(E::custom).map(JsonValue::U64)
    }

    #[inline]
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Self::Value::String(v.to_string()))
    }

    #[inline]
    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Self::Value::Null)
    }

    #[inline]
    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer)
    }

    fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        Deserialize::deserialize(SeqAccessDeserializer::new(seq)).map(Self::Value::Array)
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut entries = vec![];

        while let Some((key, value)) = map.next_entry()? {
            entries.push((key, value))
        }

        Ok(Self::Value::Object(entries))
    }
}
