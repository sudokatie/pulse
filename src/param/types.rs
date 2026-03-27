//! Parameter type definitions

/// Parameter types
#[derive(Debug, Clone)]
pub enum ParamType {
    /// Floating point (min, max, default)
    Float { min: f32, max: f32, default: f32 },
    /// Integer (min, max, default)
    Int { min: i32, max: i32, default: i32 },
    /// Boolean
    Bool { default: bool },
    /// Enumeration (choices, default index)
    Enum { choices: Vec<String>, default: usize },
}

/// Parameter information
#[derive(Debug, Clone)]
pub struct ParamInfo {
    pub id: u32,
    pub name: String,
    pub param_type: ParamType,
    pub unit: Option<String>,
}

impl ParamInfo {
    /// Create float parameter
    pub fn float(id: u32, name: &str, min: f32, max: f32, default: f32) -> Self {
        Self {
            id,
            name: name.to_string(),
            param_type: ParamType::Float { min, max, default },
            unit: None,
        }
    }

    /// Create float parameter with unit
    pub fn float_with_unit(id: u32, name: &str, min: f32, max: f32, default: f32, unit: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            param_type: ParamType::Float { min, max, default },
            unit: Some(unit.to_string()),
        }
    }

    /// Create int parameter
    pub fn int(id: u32, name: &str, min: i32, max: i32, default: i32) -> Self {
        Self {
            id,
            name: name.to_string(),
            param_type: ParamType::Int { min, max, default },
            unit: None,
        }
    }

    /// Create bool parameter
    pub fn bool(id: u32, name: &str, default: bool) -> Self {
        Self {
            id,
            name: name.to_string(),
            param_type: ParamType::Bool { default },
            unit: None,
        }
    }

    /// Create enum parameter
    pub fn enumeration(id: u32, name: &str, choices: &[&str], default: usize) -> Self {
        Self {
            id,
            name: name.to_string(),
            param_type: ParamType::Enum {
                choices: choices.iter().map(|s| s.to_string()).collect(),
                default,
            },
            unit: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_param_info_float() {
        let p = ParamInfo::float(0, "Volume", 0.0, 1.0, 0.5);
        assert_eq!(p.name, "Volume");
        assert!(matches!(p.param_type, ParamType::Float { .. }));
    }

    #[test]
    fn test_param_info_int() {
        let p = ParamInfo::int(1, "Octave", -2, 2, 0);
        assert_eq!(p.name, "Octave");
        assert!(matches!(p.param_type, ParamType::Int { .. }));
    }

    #[test]
    fn test_param_info_bool() {
        let p = ParamInfo::bool(2, "Enabled", true);
        assert!(matches!(p.param_type, ParamType::Bool { default: true }));
    }

    #[test]
    fn test_param_info_enum() {
        let p = ParamInfo::enumeration(3, "Mode", &["A", "B", "C"], 0);
        if let ParamType::Enum { choices, default } = p.param_type {
            assert_eq!(choices.len(), 3);
            assert_eq!(default, 0);
        } else {
            panic!("Expected Enum");
        }
    }
}
