//! Parameter values and normalization

use serde::{Deserialize, Serialize};
use super::types::ParamType;

/// Parameter value
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ParamValue {
    Float(f32),
    Int(i32),
    Bool(bool),
    Enum(usize),
}

impl ParamValue {
    /// Normalize value to 0.0-1.0 range
    pub fn normalize(&self, param_type: &ParamType) -> f32 {
        match (self, param_type) {
            (ParamValue::Float(v), ParamType::Float { min, max, .. }) => {
                (v - min) / (max - min)
            }
            (ParamValue::Int(v), ParamType::Int { min, max, .. }) => {
                (*v as f32 - *min as f32) / (*max as f32 - *min as f32)
            }
            (ParamValue::Bool(v), _) => if *v { 1.0 } else { 0.0 },
            (ParamValue::Enum(v), ParamType::Enum { choices, .. }) => {
                if choices.is_empty() {
                    0.0
                } else {
                    *v as f32 / (choices.len() - 1).max(1) as f32
                }
            }
            _ => 0.0,
        }
    }

    /// Create value from normalized 0.0-1.0 range
    pub fn denormalize(normalized: f32, param_type: &ParamType) -> Self {
        let n = normalized.clamp(0.0, 1.0);
        match param_type {
            ParamType::Float { min, max, .. } => {
                ParamValue::Float(min + n * (max - min))
            }
            ParamType::Int { min, max, .. } => {
                ParamValue::Int(*min + (n * (*max - *min) as f32).round() as i32)
            }
            ParamType::Bool { .. } => {
                ParamValue::Bool(n >= 0.5)
            }
            ParamType::Enum { choices, .. } => {
                let idx = (n * (choices.len() - 1).max(1) as f32).round() as usize;
                ParamValue::Enum(idx.min(choices.len().saturating_sub(1)))
            }
        }
    }

    /// Get float value (panics if not float)
    pub fn as_float(&self) -> f32 {
        match self {
            ParamValue::Float(v) => *v,
            _ => panic!("Not a float parameter"),
        }
    }

    /// Get int value (panics if not int)
    pub fn as_int(&self) -> i32 {
        match self {
            ParamValue::Int(v) => *v,
            _ => panic!("Not an int parameter"),
        }
    }

    /// Get bool value (panics if not bool)
    pub fn as_bool(&self) -> bool {
        match self {
            ParamValue::Bool(v) => *v,
            _ => panic!("Not a bool parameter"),
        }
    }

    /// Get enum index (panics if not enum)
    pub fn as_enum(&self) -> usize {
        match self {
            ParamValue::Enum(v) => *v,
            _ => panic!("Not an enum parameter"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_float_normalize() {
        let pt = ParamType::Float { min: 0.0, max: 100.0, default: 50.0 };
        let v = ParamValue::Float(50.0);
        assert!((v.normalize(&pt) - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_float_denormalize() {
        let pt = ParamType::Float { min: 0.0, max: 100.0, default: 50.0 };
        let v = ParamValue::denormalize(0.5, &pt);
        assert!((v.as_float() - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_int_normalize() {
        let pt = ParamType::Int { min: 0, max: 10, default: 5 };
        let v = ParamValue::Int(5);
        assert!((v.normalize(&pt) - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_bool_normalize() {
        let pt = ParamType::Bool { default: false };
        assert_eq!(ParamValue::Bool(true).normalize(&pt), 1.0);
        assert_eq!(ParamValue::Bool(false).normalize(&pt), 0.0);
    }

    #[test]
    fn test_enum_normalize() {
        let pt = ParamType::Enum { choices: vec!["A".into(), "B".into(), "C".into()], default: 0 };
        let v = ParamValue::Enum(1);
        assert!((v.normalize(&pt) - 0.5).abs() < 0.001);
    }
}
