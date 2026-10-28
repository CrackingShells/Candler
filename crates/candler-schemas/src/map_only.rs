//! A reader that takes a struct from a map and from nothing else.

use std::borrow::Cow;
use std::fmt;
use std::marker::PhantomData;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::de::value::MapAccessDeserializer;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

/// Reads a struct from a map and from nothing else, so a struct written as a list is refused.
///
/// The schema of a `MapOnly<T>` is the schema of `T`.
pub(crate) struct MapOnly<T>(pub(crate) T);

impl<'de, T> Deserialize<'de> for MapOnly<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(MapOnlyVisitor(PhantomData))
    }
}

/// The visitor of [`MapOnly`]: it implements `visit_map` and no other `visit_*`.
struct MapOnlyVisitor<T>(PhantomData<T>);

impl<'de, T> Visitor<'de> for MapOnlyVisitor<T>
where
    T: Deserialize<'de>,
{
    type Value = MapOnly<T>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an object")
    }

    fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        T::deserialize(MapAccessDeserializer::new(map)).map(MapOnly)
    }
}

impl<T: JsonSchema> JsonSchema for MapOnly<T> {
    fn inline_schema() -> bool {
        T::inline_schema()
    }

    fn schema_name() -> Cow<'static, str> {
        T::schema_name()
    }

    fn schema_id() -> Cow<'static, str> {
        T::schema_id()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        T::json_schema(generator)
    }
}
