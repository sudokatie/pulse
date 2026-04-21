//! VST3 parameter mapping - bidirectional mapping between Pulse params and VST3 indices

use std::collections::HashMap;

use crate::param::{ParamInfo, ParamType};

/// Mapping between Pulse parameter IDs and VST3 indices with value conversion
pub struct Vst3ParameterMapping {
    /// Map from Pulse param_id to VST3 index
    id_to_index: HashMap<u32, u32>,
    /// Map from VST3 index to Pulse param_id
    index_to_id: HashMap<u32, u32>,
    /// Parameter info for value conversion
    param_info: HashMap<u32, ParamInfo>,
}

impl Vst3ParameterMapping {
    /// Create a new empty mapping
    pub fn new() -> Self {
        Self {
            id_to_index: HashMap::new(),
            index_to_id: HashMap::new(),
            param_info: HashMap::new(),
        }
    }

    /// Build mapping from a list of parameter infos
    pub fn from_params(params: &[ParamInfo]) -> Self {
        let mut mapping = Self::new();
        for (index, param) in params.iter().enumerate() {
            mapping.add_mapping(param.id, index as u32, param.clone());
        }
        mapping
    }

    /// Add a mapping between param_id and vst3_index
    pub fn add_mapping(&mut self, param_id: u32, vst3_index: u32, info: ParamInfo) {
        self.id_to_index.insert(param_id, vst3_index);
        self.index_to_id.insert(vst3_index, param_id);
        self.param_info.insert(param_id, info);
    }

    /// Get VST3 index from Pulse param_id
    pub fn param_id_to_vst3_index(&self, param_id: u32) -> Option<u32> {
        self.id_to_index.get(&param_id).copied()
    }

    /// Get Pulse param_id from VST3 index
    pub fn vst3_index_to_param_id(&self, vst3_index: u32) -> Option<u32> {
        self.index_to_id.get(&vst3_index).copied()
    }

    /// Get parameter info by param_id
    pub fn get_param_info(&self, param_id: u32) -> Option<&ParamInfo> {
        self.param_info.get(&param_id)
    }

    /// Get parameter info by VST3 index
    pub fn get_param_info_by_index(&self, vst3_index: u32) -> Option<&ParamInfo> {
        self.vst3_index_to_param_id(vst3_index)
            .and_then(|id| self.param_info.get(&id))
    }

    /// Get total number of parameters
    pub fn count(&self) -> usize {
        self.id_to_index.len()
    }

    /// Convert plain value to normalized (0-1)
    pub fn plain_to_normalized(&self, param_id: u32, plain_value: f64) -> f64 {
        if let Some(info) = self.param_info.get(&param_id) {
            match &info.param_type {
                ParamType::Float { min, max, .. } => {
                    let range = (*max as f64) - (*min as f64);
                    if range == 0.0 {
                        0.0
                    } else {
                        ((plain_value - *min as f64) / range).clamp(0.0, 1.0)
                    }
                }
                ParamType::Int { min, max, .. } => {
                    let range = (*max - *min) as f64;
                    if range == 0.0 {
                        0.0
                    } else {
                        ((plain_value - *min as f64) / range).clamp(0.0, 1.0)
                    }
                }
                ParamType::Bool { .. } => {
                    if plain_value >= 0.5 { 1.0 } else { 0.0 }
                }
                ParamType::Enum { choices, .. } => {
                    let count = choices.len() as f64;
                    if count <= 1.0 {
                        0.0
                    } else {
                        (plain_value / (count - 1.0)).clamp(0.0, 1.0)
                    }
                }
            }
        } else {
            plain_value.clamp(0.0, 1.0)
        }
    }

    /// Convert normalized (0-1) value to plain value
    pub fn normalized_to_plain(&self, param_id: u32, normalized_value: f64) -> f64 {
        if let Some(info) = self.param_info.get(&param_id) {
            match &info.param_type {
                ParamType::Float { min, max, .. } => {
                    let range = (*max as f64) - (*min as f64);
                    *min as f64 + normalized_value.clamp(0.0, 1.0) * range
                }
                ParamType::Int { min, max, .. } => {
                    let range = (*max - *min) as f64;
                    let plain = *min as f64 + normalized_value.clamp(0.0, 1.0) * range;
                    plain.round()
                }
                ParamType::Bool { .. } => {
                    if normalized_value >= 0.5 { 1.0 } else { 0.0 }
                }
                ParamType::Enum { choices, .. } => {
                    let count = choices.len();
                    if count <= 1 {
                        0.0
                    } else {
                        let index = (normalized_value.clamp(0.0, 1.0) * (count - 1) as f64).round();
                        index
                    }
                }
            }
        } else {
            normalized_value
        }
    }

    /// Get the default normalized value for a parameter
    pub fn get_default_normalized(&self, param_id: u32) -> f64 {
        if let Some(info) = self.param_info.get(&param_id) {
            match &info.param_type {
                ParamType::Float { min, max, default } => {
                    let range = (*max as f64) - (*min as f64);
                    if range == 0.0 {
                        0.0
                    } else {
                        ((*default as f64) - (*min as f64)) / range
                    }
                }
                ParamType::Int { min, max, default } => {
                    let range = (*max - *min) as f64;
                    if range == 0.0 {
                        0.0
                    } else {
                        ((*default - *min) as f64) / range
                    }
                }
                ParamType::Bool { default } => {
                    if *default { 1.0 } else { 0.0 }
                }
                ParamType::Enum { choices, default } => {
                    let count = choices.len();
                    if count <= 1 {
                        0.0
                    } else {
                        (*default as f64) / (count - 1) as f64
                    }
                }
            }
        } else {
            0.0
        }
    }

    /// Get step count for parameter (0 = continuous)
    pub fn get_step_count(&self, param_id: u32) -> i32 {
        if let Some(info) = self.param_info.get(&param_id) {
            match &info.param_type {
                ParamType::Float { .. } => 0,  // Continuous
                ParamType::Int { min, max, .. } => (*max - *min).max(0),
                ParamType::Bool { .. } => 1,  // 2 states = 1 step
                ParamType::Enum { choices, .. } => (choices.len() as i32 - 1).max(0),
            }
        } else {
            0
        }
    }
}

impl Default for Vst3ParameterMapping {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_mapping() -> Vst3ParameterMapping {
        let params = vec![
            ParamInfo::float(0, "Volume", 0.0, 1.0, 0.5),
            ParamInfo::float(1, "Frequency", 20.0, 20000.0, 1000.0),
            ParamInfo::int(2, "Octave", -2, 2, 0),
            ParamInfo::bool(3, "Bypass", false),
            ParamInfo::enumeration(4, "Mode", &["A", "B", "C"], 1),
        ];
        Vst3ParameterMapping::from_params(&params)
    }

    #[test]
    fn test_param_id_to_vst3_index() {
        let mapping = create_test_mapping();

        assert_eq!(mapping.param_id_to_vst3_index(0), Some(0));
        assert_eq!(mapping.param_id_to_vst3_index(1), Some(1));
        assert_eq!(mapping.param_id_to_vst3_index(4), Some(4));
        assert_eq!(mapping.param_id_to_vst3_index(99), None);
    }

    #[test]
    fn test_vst3_index_to_param_id() {
        let mapping = create_test_mapping();

        assert_eq!(mapping.vst3_index_to_param_id(0), Some(0));
        assert_eq!(mapping.vst3_index_to_param_id(2), Some(2));
        assert_eq!(mapping.vst3_index_to_param_id(99), None);
    }

    #[test]
    fn test_float_plain_to_normalized() {
        let mapping = create_test_mapping();

        // Volume: 0.0 - 1.0
        assert!((mapping.plain_to_normalized(0, 0.0) - 0.0).abs() < 0.001);
        assert!((mapping.plain_to_normalized(0, 0.5) - 0.5).abs() < 0.001);
        assert!((mapping.plain_to_normalized(0, 1.0) - 1.0).abs() < 0.001);

        // Frequency: 20.0 - 20000.0
        assert!((mapping.plain_to_normalized(1, 20.0) - 0.0).abs() < 0.001);
        assert!((mapping.plain_to_normalized(1, 20000.0) - 1.0).abs() < 0.001);
        // 1000 Hz should be approximately 0.049
        let norm = mapping.plain_to_normalized(1, 1000.0);
        assert!((norm - (980.0 / 19980.0)).abs() < 0.001);
    }

    #[test]
    fn test_float_normalized_to_plain() {
        let mapping = create_test_mapping();

        // Volume: 0.0 - 1.0
        assert!((mapping.normalized_to_plain(0, 0.0) - 0.0).abs() < 0.001);
        assert!((mapping.normalized_to_plain(0, 0.5) - 0.5).abs() < 0.001);
        assert!((mapping.normalized_to_plain(0, 1.0) - 1.0).abs() < 0.001);

        // Frequency: 20.0 - 20000.0
        assert!((mapping.normalized_to_plain(1, 0.0) - 20.0).abs() < 0.001);
        assert!((mapping.normalized_to_plain(1, 1.0) - 20000.0).abs() < 0.001);
    }

    #[test]
    fn test_int_plain_to_normalized() {
        let mapping = create_test_mapping();

        // Octave: -2 to 2 (5 values)
        assert!((mapping.plain_to_normalized(2, -2.0) - 0.0).abs() < 0.001);
        assert!((mapping.plain_to_normalized(2, 0.0) - 0.5).abs() < 0.001);
        assert!((mapping.plain_to_normalized(2, 2.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_int_normalized_to_plain() {
        let mapping = create_test_mapping();

        // Octave: -2 to 2
        assert!((mapping.normalized_to_plain(2, 0.0) - (-2.0)).abs() < 0.001);
        assert!((mapping.normalized_to_plain(2, 0.5) - 0.0).abs() < 0.001);
        assert!((mapping.normalized_to_plain(2, 1.0) - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_bool_conversion() {
        let mapping = create_test_mapping();

        // Bool to normalized
        assert!((mapping.plain_to_normalized(3, 0.0) - 0.0).abs() < 0.001);
        assert!((mapping.plain_to_normalized(3, 1.0) - 1.0).abs() < 0.001);

        // Normalized to bool
        assert!((mapping.normalized_to_plain(3, 0.0) - 0.0).abs() < 0.001);
        assert!((mapping.normalized_to_plain(3, 0.4) - 0.0).abs() < 0.001);
        assert!((mapping.normalized_to_plain(3, 0.6) - 1.0).abs() < 0.001);
        assert!((mapping.normalized_to_plain(3, 1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_enum_conversion() {
        let mapping = create_test_mapping();

        // Enum with 3 choices: A=0, B=1, C=2
        assert!((mapping.plain_to_normalized(4, 0.0) - 0.0).abs() < 0.001);
        assert!((mapping.plain_to_normalized(4, 1.0) - 0.5).abs() < 0.001);
        assert!((mapping.plain_to_normalized(4, 2.0) - 1.0).abs() < 0.001);

        assert!((mapping.normalized_to_plain(4, 0.0) - 0.0).abs() < 0.001);
        assert!((mapping.normalized_to_plain(4, 0.5) - 1.0).abs() < 0.001);
        assert!((mapping.normalized_to_plain(4, 1.0) - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_roundtrip_conversion() {
        let mapping = create_test_mapping();

        // Float roundtrip
        for plain in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let norm = mapping.plain_to_normalized(0, plain);
            let back = mapping.normalized_to_plain(0, norm);
            assert!((back - plain).abs() < 0.001, "Float roundtrip failed for {}", plain);
        }

        // Int roundtrip
        for plain in [-2.0, -1.0, 0.0, 1.0, 2.0] {
            let norm = mapping.plain_to_normalized(2, plain);
            let back = mapping.normalized_to_plain(2, norm);
            assert!((back - plain).abs() < 0.001, "Int roundtrip failed for {}", plain);
        }

        // Enum roundtrip
        for plain in [0.0, 1.0, 2.0] {
            let norm = mapping.plain_to_normalized(4, plain);
            let back = mapping.normalized_to_plain(4, norm);
            assert!((back - plain).abs() < 0.001, "Enum roundtrip failed for {}", plain);
        }
    }

    #[test]
    fn test_get_default_normalized() {
        let mapping = create_test_mapping();

        // Volume default: 0.5 in range 0-1
        assert!((mapping.get_default_normalized(0) - 0.5).abs() < 0.001);

        // Frequency default: 1000 in range 20-20000
        let expected = (1000.0 - 20.0) / (20000.0 - 20.0);
        assert!((mapping.get_default_normalized(1) - expected).abs() < 0.001);

        // Octave default: 0 in range -2 to 2
        assert!((mapping.get_default_normalized(2) - 0.5).abs() < 0.001);

        // Bypass default: false
        assert!((mapping.get_default_normalized(3) - 0.0).abs() < 0.001);

        // Mode default: 1 (B) out of 3 choices
        assert!((mapping.get_default_normalized(4) - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_get_step_count() {
        let mapping = create_test_mapping();

        // Float: continuous
        assert_eq!(mapping.get_step_count(0), 0);

        // Int: -2 to 2 = 4 steps
        assert_eq!(mapping.get_step_count(2), 4);

        // Bool: 1 step (2 states)
        assert_eq!(mapping.get_step_count(3), 1);

        // Enum: 3 choices = 2 steps
        assert_eq!(mapping.get_step_count(4), 2);
    }

    #[test]
    fn test_get_param_info() {
        let mapping = create_test_mapping();

        let info = mapping.get_param_info(0).unwrap();
        assert_eq!(info.name, "Volume");

        let info = mapping.get_param_info_by_index(1).unwrap();
        assert_eq!(info.name, "Frequency");

        assert!(mapping.get_param_info(99).is_none());
        assert!(mapping.get_param_info_by_index(99).is_none());
    }

    #[test]
    fn test_count() {
        let mapping = create_test_mapping();
        assert_eq!(mapping.count(), 5);

        let empty = Vst3ParameterMapping::new();
        assert_eq!(empty.count(), 0);
    }

    #[test]
    fn test_clamping() {
        let mapping = create_test_mapping();

        // Values outside range should be clamped
        assert!((mapping.plain_to_normalized(0, -1.0) - 0.0).abs() < 0.001);
        assert!((mapping.plain_to_normalized(0, 2.0) - 1.0).abs() < 0.001);

        assert!((mapping.normalized_to_plain(0, -0.5) - 0.0).abs() < 0.001);
        assert!((mapping.normalized_to_plain(0, 1.5) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_custom_mapping() {
        let mut mapping = Vst3ParameterMapping::new();

        // Add parameters with non-sequential IDs
        mapping.add_mapping(100, 0, ParamInfo::float(100, "Param A", 0.0, 10.0, 5.0));
        mapping.add_mapping(200, 1, ParamInfo::float(200, "Param B", 0.0, 100.0, 50.0));

        assert_eq!(mapping.param_id_to_vst3_index(100), Some(0));
        assert_eq!(mapping.param_id_to_vst3_index(200), Some(1));
        assert_eq!(mapping.vst3_index_to_param_id(0), Some(100));
        assert_eq!(mapping.vst3_index_to_param_id(1), Some(200));
    }
}
