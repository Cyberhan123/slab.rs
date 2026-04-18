use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A strongly-typed, opaque identifier backed by a [`Uuid`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Id(Uuid);

impl Id {
    /// Generate a new random identifier.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Wrap an existing [`Uuid`].
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Return the inner [`Uuid`].
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for Id {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// An RFC 3339 UTC timestamp.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct Timestamp(DateTime<Utc>);

impl Timestamp {
    /// Return the current wall-clock time.
    pub fn now() -> Self {
        Self(Utc::now())
    }

    /// Wrap an existing [`DateTime<Utc>`].
    pub fn from_utc(dt: DateTime<Utc>) -> Self {
        Self(dt)
    }

    /// Return the inner [`DateTime<Utc>`].
    pub fn as_datetime(&self) -> DateTime<Utc> {
        self.0
    }
}

impl Default for Timestamp {
    fn default() -> Self {
        Self::now()
    }
}

impl std::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_serialization_roundtrip() {
        let id = Id::new();
        let json = serde_json::to_string(&id).expect("serialize Id");
        let deserialized: Id = serde_json::from_str(&json).expect("deserialize Id");
        assert_eq!(id, deserialized);
    }

    #[test]
    fn timestamp_serialization_roundtrip() {
        let ts = Timestamp::now();
        let json = serde_json::to_string(&ts).expect("serialize Timestamp");
        let deserialized: Timestamp = serde_json::from_str(&json).expect("deserialize Timestamp");
        assert_eq!(ts, deserialized);
    }

    #[test]
    fn id_from_uuid_roundtrip() {
        let uuid = Uuid::new_v4();
        let id = Id::from_uuid(uuid);
        assert_eq!(id.as_uuid(), uuid);
    }

    #[test]
    fn timestamp_from_utc_roundtrip() {
        let dt = Utc::now();
        let ts = Timestamp::from_utc(dt);
        assert_eq!(ts.as_datetime(), dt);
    }

    // Property-based tests
    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig {
            cases: 256,
            .. Default::default()
        })]

        #[test]
        fn id_serialization_roundtrip_prop(uuid_bytes in proptest::array::uniform16(proptest::num::u8::ANY)) {
            let uuid = Uuid::from_bytes(uuid_bytes);
            let id = Id::from_uuid(uuid);
            let json = serde_json::to_string(&id).unwrap();
            let deserialized: Id = serde_json::from_str(&json).unwrap();
            proptest::prop_assert_eq!(id, deserialized);
        }

        #[test]
        fn id_display_parse_roundprop(uuid_bytes in proptest::array::uniform16(proptest::num::u8::ANY)) {
            let uuid = Uuid::from_bytes(uuid_bytes);
            let id = Id::from_uuid(uuid);
            let display = id.to_string();
            let parsed = display.parse::<Uuid>().unwrap();
            proptest::prop_assert_eq!(id.as_uuid(), parsed);
        }

        #[test]
        fn timestamp_json_roundtree(timestamp_millis in -2208988800000i64..=253402300799000i64) {
            // Convert milliseconds to DateTime
            let secs = timestamp_millis / 1000;
            let nsecs = ((timestamp_millis % 1000).abs() * 1_000_000) as u32;
            let dt = DateTime::from_timestamp(secs, nsecs).unwrap();
            let ts = Timestamp::from_utc(dt);

            let json = serde_json::to_string(&ts).unwrap();
            let deserialized: Timestamp = serde_json::from_str(&json).unwrap();
            proptest::prop_assert_eq!(ts, deserialized);
        }
    }
}
