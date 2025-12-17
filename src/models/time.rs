//! Time value types for Slurm JSON responses.
//!
//! This module defines types for handling Slurm's time value representation,
//! which uses set/infinite/number fields to represent optional and unlimited values.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Slurm time value - represents optional/infinite numeric values from Slurm JSON.
///
/// This enum ensures that only valid states are representable:
/// - `NotSet`: The value was not set in Slurm (set=false)
/// - `Infinite`: The value represents infinity (set=true, infinite=true)
/// - `Value(u64)`: A concrete numeric value (set=true, infinite=false)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum TimeValue {
    /// Value not set (Slurm JSON: set=false)
    #[default]
    NotSet,
    /// Infinite/unlimited value (Slurm JSON: set=true, infinite=true)
    Infinite,
    /// Concrete numeric value (Slurm JSON: set=true, infinite=false, number=N)
    Value(u64),
}

impl TimeValue {
    /// Returns the numeric value if set and not infinite.
    #[must_use]
    pub fn value(&self) -> Option<u64> {
        match self {
            TimeValue::Value(n) => Some(*n),
            _ => None,
        }
    }

    /// Returns the numeric value, or 0 if not set or infinite.
    /// Useful for backwards-compatible field access patterns.
    #[must_use]
    pub fn number(&self) -> u64 {
        match self {
            TimeValue::Value(n) => *n,
            _ => 0,
        }
    }

    /// Returns true if this value is set (either Value or Infinite).
    #[allow(dead_code)]
    #[must_use]
    pub fn is_set(&self) -> bool {
        !matches!(self, TimeValue::NotSet)
    }

    /// Returns true if this value represents infinity.
    #[allow(dead_code)]
    #[must_use]
    pub fn is_infinite(&self) -> bool {
        matches!(self, TimeValue::Infinite)
    }

    /// Convert to a timestamp if this is a concrete value.
    #[allow(dead_code)]
    #[must_use]
    pub fn to_timestamp(&self) -> Option<DateTime<Utc>> {
        match self {
            TimeValue::Value(n) => DateTime::from_timestamp(*n as i64, 0),
            _ => None,
        }
    }

    /// Create a TimeValue from explicit set/infinite/number fields.
    /// Used internally for deserialization.
    #[must_use]
    fn from_fields(set: bool, infinite: bool, number: u64) -> Self {
        if !set {
            TimeValue::NotSet
        } else if infinite {
            TimeValue::Infinite
        } else {
            TimeValue::Value(number)
        }
    }
}

/// Internal struct for deserializing Slurm's JSON format
#[derive(Deserialize)]
struct TimeValueRaw {
    #[serde(default)]
    set: bool,
    #[serde(default)]
    infinite: bool,
    #[serde(default)]
    number: u64,
}

impl<'de> Deserialize<'de> for TimeValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = TimeValueRaw::deserialize(deserializer)?;
        Ok(TimeValue::from_fields(raw.set, raw.infinite, raw.number))
    }
}

impl Serialize for TimeValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("TimeValue", 3)?;
        match self {
            TimeValue::NotSet => {
                state.serialize_field("set", &false)?;
                state.serialize_field("infinite", &false)?;
                state.serialize_field("number", &0u64)?;
            }
            TimeValue::Infinite => {
                state.serialize_field("set", &true)?;
                state.serialize_field("infinite", &true)?;
                state.serialize_field("number", &0u64)?;
            }
            TimeValue::Value(n) => {
                state.serialize_field("set", &true)?;
                state.serialize_field("infinite", &false)?;
                state.serialize_field("number", n)?;
            }
        }
        state.end()
    }
}

/// Slurm floating-point value structure (used by sshare for normalized values)
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct FloatValue {
    #[serde(default)]
    pub set: bool,
    #[serde(default)]
    pub infinite: bool,
    #[serde(default)]
    pub number: f64,
}

impl FloatValue {
    #[allow(dead_code)]
    #[must_use]
    pub fn value(&self) -> Option<f64> {
        if self.set && !self.infinite {
            Some(self.number)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_value_timestamp() {
        let tv = TimeValue::Value(1704067200);
        let ts = tv.to_timestamp();
        assert!(ts.is_some());

        let tv_infinite = TimeValue::Infinite;
        assert!(tv_infinite.to_timestamp().is_none());

        let tv_unset = TimeValue::NotSet;
        assert!(tv_unset.to_timestamp().is_none());
    }

    #[test]
    fn test_time_value_enum_methods() {
        // Test Value variant
        let val = TimeValue::Value(42);
        assert!(val.is_set());
        assert!(!val.is_infinite());
        assert_eq!(val.value(), Some(42));
        assert_eq!(val.number(), 42);

        // Test Infinite variant
        let inf = TimeValue::Infinite;
        assert!(inf.is_set());
        assert!(inf.is_infinite());
        assert_eq!(inf.value(), None);
        assert_eq!(inf.number(), 0);

        // Test NotSet variant
        let unset = TimeValue::NotSet;
        assert!(!unset.is_set());
        assert!(!unset.is_infinite());
        assert_eq!(unset.value(), None);
        assert_eq!(unset.number(), 0);

        // Test Default
        assert_eq!(TimeValue::default(), TimeValue::NotSet);
    }

    #[test]
    fn test_time_value_serde_roundtrip() {
        // Test Value variant
        let val = TimeValue::Value(12345);
        let json = serde_json::to_string(&val).unwrap();
        assert!(json.contains("\"set\":true"));
        assert!(json.contains("\"infinite\":false"));
        assert!(json.contains("\"number\":12345"));
        let parsed: TimeValue = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, val);

        // Test Infinite variant
        let inf = TimeValue::Infinite;
        let json = serde_json::to_string(&inf).unwrap();
        assert!(json.contains("\"set\":true"));
        assert!(json.contains("\"infinite\":true"));
        let parsed: TimeValue = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, inf);

        // Test NotSet variant
        let unset = TimeValue::NotSet;
        let json = serde_json::to_string(&unset).unwrap();
        assert!(json.contains("\"set\":false"));
        let parsed: TimeValue = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, unset);
    }
}
