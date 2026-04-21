//! VST3 FFI types and constants

use std::ffi::c_void;

/// VST3 TUID - 16-byte class identifier
pub type TUID = [u8; 16];

/// VST3 FIDString - null-terminated C string
pub type FIDString = *const i8;

/// VST3 result type
pub type TResult = i32;

// Result codes
pub const K_RESULT_OK: TResult = 0;
pub const K_RESULT_TRUE: TResult = 0;
pub const K_RESULT_FALSE: TResult = 1;
pub const K_INVALID_ARGUMENT: TResult = 2;
pub const K_NOT_IMPLEMENTED: TResult = 3;
pub const K_INTERNAL_ERROR: TResult = 4;
pub const K_NOT_INITIALIZED: TResult = 5;
pub const K_OUT_OF_MEMORY: TResult = 6;

// Media types
pub const K_AUDIO: i32 = 0;
pub const K_EVENT: i32 = 1;

// Bus directions
pub const K_INPUT: i32 = 0;
pub const K_OUTPUT: i32 = 1;

// Bus types
pub const K_MAIN: i32 = 0;
pub const K_AUX: i32 = 1;

// Process modes
pub const K_REALTIME: i32 = 0;
pub const K_PREFETCH: i32 = 1;
pub const K_OFFLINE: i32 = 2;

// Symbolic sample sizes
pub const K_SAMPLE_32: i32 = 0;
pub const K_SAMPLE_64: i32 = 1;

// Speaker arrangements (common)
pub const K_SPEAKER_L: u64 = 1 << 0;
pub const K_SPEAKER_R: u64 = 1 << 1;
pub const K_SPEAKER_C: u64 = 1 << 2;
pub const K_SPEAKER_LFE: u64 = 1 << 3;
pub const K_SPEAKER_LS: u64 = 1 << 4;
pub const K_SPEAKER_RS: u64 = 1 << 5;

// Common speaker arrangements
pub const K_MONO: u64 = K_SPEAKER_C;
pub const K_STEREO: u64 = K_SPEAKER_L | K_SPEAKER_R;

// Parameter flags
pub const K_CAN_AUTOMATE: i32 = 1 << 0;
pub const K_IS_READ_ONLY: i32 = 1 << 1;
pub const K_IS_WRAP_AROUND: i32 = 1 << 2;
pub const K_IS_LIST: i32 = 1 << 3;
pub const K_IS_HIDDEN: i32 = 1 << 4;
pub const K_IS_PROGRAM_CHANGE: i32 = 1 << 15;
pub const K_IS_BYPASS: i32 = 1 << 16;

// Factory flags
pub const K_NO_FLAGS: i32 = 0;
pub const K_CLASSES_DISCARDABLE: i32 = 1 << 0;
pub const K_LICENSE_CHECK: i32 = 1 << 1;
pub const K_COMPONENT_NON_DISCARDABLE: i32 = 1 << 3;
pub const K_UNICODE: i32 = 1 << 4;

// Class cardinality
pub const K_MANY_INSTANCES: i32 = 0x7FFFFFFF;

// Component flags
pub const K_DISTRIBUTABLE: u32 = 1 << 0;
pub const K_SIMPLE_MODE_SUPPORTED: u32 = 1 << 1;

// Standard VST3 interface IDs
pub mod iid {
    use super::TUID;

    /// IUnknown / FUnknown IID
    pub const FUNKNOWN: TUID = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46
    ];

    /// IPluginFactory IID
    pub const IPLUGIN_FACTORY: TUID = [
        0x7A, 0x4D, 0x81, 0x1C, 0x52, 0x11, 0x45, 0x4F,
        0x86, 0xF9, 0x21, 0x66, 0x54, 0x18, 0x85, 0xF0
    ];

    /// IPluginFactory2 IID
    pub const IPLUGIN_FACTORY2: TUID = [
        0x0D, 0x11, 0x3F, 0xCF, 0x87, 0x63, 0x45, 0x7B,
        0xA5, 0x40, 0x3C, 0x06, 0xF0, 0x8A, 0x43, 0x2A
    ];

    /// IPluginFactory3 IID
    pub const IPLUGIN_FACTORY3: TUID = [
        0x4F, 0xC3, 0x24, 0x88, 0x47, 0x50, 0x4E, 0xF8,
        0x8C, 0x76, 0x12, 0x9F, 0xCA, 0xAB, 0x47, 0x06
    ];

    /// IPluginBase IID
    pub const IPLUGIN_BASE: TUID = [
        0x22, 0x88, 0x88, 0x88, 0x00, 0x00, 0x00, 0x00,
        0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46
    ];

    /// IComponent IID
    pub const ICOMPONENT: TUID = [
        0xE8, 0x31, 0xFF, 0x31, 0xF2, 0xD5, 0x41, 0x01,
        0x92, 0x8E, 0xBB, 0xEE, 0x25, 0x69, 0x78, 0x02
    ];

    /// IAudioProcessor IID
    pub const IAUDIO_PROCESSOR: TUID = [
        0x42, 0x04, 0x3F, 0x99, 0xB7, 0xDA, 0x45, 0x3C,
        0xA5, 0x69, 0xE7, 0x9D, 0x9A, 0xAE, 0xC3, 0x3D
    ];

    /// IEditController IID
    pub const IEDIT_CONTROLLER: TUID = [
        0xDB, 0xA5, 0x13, 0x3A, 0xDA, 0x14, 0x41, 0x5B,
        0xAC, 0xDA, 0x13, 0x51, 0x82, 0x27, 0x78, 0x15
    ];

    /// IUnitInfo IID
    pub const IUNIT_INFO: TUID = [
        0x3D, 0x4B, 0xD6, 0xB5, 0x91, 0x3A, 0x4F, 0xD2,
        0xA8, 0x86, 0xE7, 0x68, 0xA5, 0xEB, 0x92, 0xC1
    ];

    /// IConnectionPoint IID
    pub const ICONNECTION_POINT: TUID = [
        0x7F, 0x4E, 0xE6, 0x8F, 0x80, 0x67, 0x4B, 0xB2,
        0x95, 0x92, 0x17, 0xE6, 0x9A, 0xC2, 0xF3, 0x6C
    ];
}

/// VST3 category strings
pub mod categories {
    pub const FX: &str = "Fx";
    pub const FX_ANALYZER: &str = "Fx|Analyzer";
    pub const FX_DELAY: &str = "Fx|Delay";
    pub const FX_DISTORTION: &str = "Fx|Distortion";
    pub const FX_DYNAMICS: &str = "Fx|Dynamics";
    pub const FX_EQ: &str = "Fx|EQ";
    pub const FX_FILTER: &str = "Fx|Filter";
    pub const FX_GENERATOR: &str = "Fx|Generator";
    pub const FX_INSTRUMENT: &str = "Fx|Instrument";
    pub const FX_MODULATION: &str = "Fx|Modulation";
    pub const FX_REVERB: &str = "Fx|Reverb";
    pub const FX_SPATIAL: &str = "Fx|Spatial";
    pub const FX_TOOLS: &str = "Fx|Tools";
    pub const INSTRUMENT: &str = "Instrument";
    pub const INSTRUMENT_SYNTH: &str = "Instrument|Synth";
    pub const INSTRUMENT_SAMPLER: &str = "Instrument|Sampler";
    pub const INSTRUMENT_DRUM: &str = "Instrument|Drum";
}

/// Factory info structure
#[repr(C)]
#[derive(Debug, Clone)]
pub struct PFactoryInfo {
    pub vendor: [i8; 64],
    pub url: [i8; 256],
    pub email: [i8; 128],
    pub flags: i32,
}

impl PFactoryInfo {
    pub fn new(vendor: &str, url: &str, email: &str, flags: i32) -> Self {
        let mut info = Self {
            vendor: [0; 64],
            url: [0; 256],
            email: [0; 128],
            flags,
        };
        copy_str_to_cstr(vendor, &mut info.vendor);
        copy_str_to_cstr(url, &mut info.url);
        copy_str_to_cstr(email, &mut info.email);
        info
    }
}

impl Default for PFactoryInfo {
    fn default() -> Self {
        Self {
            vendor: [0; 64],
            url: [0; 256],
            email: [0; 128],
            flags: K_UNICODE,
        }
    }
}

/// Class info for IPluginFactory
#[repr(C)]
#[derive(Debug, Clone)]
pub struct PClassInfo {
    pub cid: TUID,
    pub cardinality: i32,
    pub category: [i8; 32],
    pub name: [i8; 64],
}

impl PClassInfo {
    pub fn new(cid: TUID, category: &str, name: &str) -> Self {
        let mut info = Self {
            cid,
            cardinality: K_MANY_INSTANCES,
            category: [0; 32],
            name: [0; 64],
        };
        copy_str_to_cstr(category, &mut info.category);
        copy_str_to_cstr(name, &mut info.name);
        info
    }
}

impl Default for PClassInfo {
    fn default() -> Self {
        Self {
            cid: [0; 16],
            cardinality: K_MANY_INSTANCES,
            category: [0; 32],
            name: [0; 64],
        }
    }
}

/// Extended class info for IPluginFactory2
#[repr(C)]
#[derive(Debug, Clone)]
pub struct PClassInfo2 {
    pub cid: TUID,
    pub cardinality: i32,
    pub category: [i8; 32],
    pub name: [i8; 64],
    pub class_flags: u32,
    pub sub_categories: [i8; 128],
    pub vendor: [i8; 64],
    pub version: [i8; 64],
    pub sdk_version: [i8; 64],
}

impl PClassInfo2 {
    pub fn new(
        cid: TUID,
        category: &str,
        name: &str,
        sub_categories: &str,
        vendor: &str,
        version: &str,
    ) -> Self {
        let mut info = Self {
            cid,
            cardinality: K_MANY_INSTANCES,
            category: [0; 32],
            name: [0; 64],
            class_flags: 0,
            sub_categories: [0; 128],
            vendor: [0; 64],
            version: [0; 64],
            sdk_version: [0; 64],
        };
        copy_str_to_cstr(category, &mut info.category);
        copy_str_to_cstr(name, &mut info.name);
        copy_str_to_cstr(sub_categories, &mut info.sub_categories);
        copy_str_to_cstr(vendor, &mut info.vendor);
        copy_str_to_cstr(version, &mut info.version);
        copy_str_to_cstr("VST 3.7", &mut info.sdk_version);
        info
    }
}

impl Default for PClassInfo2 {
    fn default() -> Self {
        Self {
            cid: [0; 16],
            cardinality: K_MANY_INSTANCES,
            category: [0; 32],
            name: [0; 64],
            class_flags: 0,
            sub_categories: [0; 128],
            vendor: [0; 64],
            version: [0; 64],
            sdk_version: [0; 64],
        }
    }
}

/// Unicode class info for IPluginFactory3
#[repr(C)]
#[derive(Debug, Clone)]
pub struct PClassInfoW {
    pub cid: TUID,
    pub cardinality: i32,
    pub category: [i8; 32],
    pub name: [u16; 64],
    pub class_flags: u32,
    pub sub_categories: [i8; 128],
    pub vendor: [u16; 64],
    pub version: [u16; 64],
    pub sdk_version: [u16; 64],
}

impl Default for PClassInfoW {
    fn default() -> Self {
        Self {
            cid: [0; 16],
            cardinality: K_MANY_INSTANCES,
            category: [0; 32],
            name: [0; 64],
            class_flags: 0,
            sub_categories: [0; 128],
            vendor: [0; 64],
            version: [0; 64],
            sdk_version: [0; 64],
        }
    }
}

/// Bus info structure
#[repr(C)]
#[derive(Debug, Clone)]
pub struct BusInfo {
    pub media_type: i32,
    pub direction: i32,
    pub channel_count: i32,
    pub name: [u16; 128],
    pub bus_type: i32,
    pub flags: u32,
}

impl Default for BusInfo {
    fn default() -> Self {
        Self {
            media_type: 0,
            direction: 0,
            channel_count: 0,
            name: [0; 128],
            bus_type: 0,
            flags: 0,
        }
    }
}

impl BusInfo {
    pub fn audio(name: &str, direction: i32, channels: i32, bus_type: i32) -> Self {
        let mut info = Self {
            media_type: K_AUDIO,
            direction,
            channel_count: channels,
            name: [0; 128],
            bus_type,
            flags: 1, // kDefaultActive
        };
        copy_str_to_u16(name, &mut info.name);
        info
    }
}

/// Routing info structure
#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct RoutingInfo {
    pub media_type: i32,
    pub bus_index: i32,
    pub channel: i32,
}

/// Parameter info structure
#[repr(C)]
#[derive(Debug, Clone)]
pub struct ParameterInfo {
    pub id: u32,
    pub title: [u16; 128],
    pub short_title: [u16; 128],
    pub units: [u16; 128],
    pub step_count: i32,
    pub default_normalized_value: f64,
    pub unit_id: i32,
    pub flags: i32,
}

impl ParameterInfo {
    pub fn new(id: u32, title: &str, default_value: f64, flags: i32) -> Self {
        let mut info = Self {
            id,
            title: [0; 128],
            short_title: [0; 128],
            units: [0; 128],
            step_count: 0,
            default_normalized_value: default_value,
            unit_id: 0,
            flags,
        };
        copy_str_to_u16(title, &mut info.title);
        info
    }

    pub fn with_unit(mut self, unit: &str) -> Self {
        copy_str_to_u16(unit, &mut self.units);
        self
    }

    pub fn with_steps(mut self, steps: i32) -> Self {
        self.step_count = steps;
        self
    }
}

impl Default for ParameterInfo {
    fn default() -> Self {
        Self {
            id: 0,
            title: [0; 128],
            short_title: [0; 128],
            units: [0; 128],
            step_count: 0,
            default_normalized_value: 0.0,
            unit_id: 0,
            flags: K_CAN_AUTOMATE,
        }
    }
}

/// Process setup structure
#[repr(C)]
#[derive(Debug, Clone)]
pub struct ProcessSetup {
    pub process_mode: i32,
    pub symbolic_sample_size: i32,
    pub max_samples_per_block: i32,
    pub sample_rate: f64,
}

impl Default for ProcessSetup {
    fn default() -> Self {
        Self {
            process_mode: K_REALTIME,
            symbolic_sample_size: K_SAMPLE_32,
            max_samples_per_block: 512,
            sample_rate: 44100.0,
        }
    }
}

/// Audio bus buffers
#[repr(C)]
#[derive(Debug)]
pub struct AudioBusBuffers {
    pub num_channels: i32,
    pub silence_flags: u64,
    pub channel_buffers32: *mut *mut f32,
    pub channel_buffers64: *mut *mut f64,
}

impl Default for AudioBusBuffers {
    fn default() -> Self {
        Self {
            num_channels: 0,
            silence_flags: 0,
            channel_buffers32: std::ptr::null_mut(),
            channel_buffers64: std::ptr::null_mut(),
        }
    }
}

/// Process data structure
#[repr(C)]
pub struct ProcessData {
    pub process_mode: i32,
    pub symbolic_sample_size: i32,
    pub num_samples: i32,
    pub num_inputs: i32,
    pub num_outputs: i32,
    pub inputs: *mut AudioBusBuffers,
    pub outputs: *mut AudioBusBuffers,
    pub input_param_changes: *mut c_void,
    pub output_param_changes: *mut c_void,
    pub input_events: *mut c_void,
    pub output_events: *mut c_void,
    pub context: *mut ProcessContext,
}

impl Default for ProcessData {
    fn default() -> Self {
        Self {
            process_mode: K_REALTIME,
            symbolic_sample_size: K_SAMPLE_32,
            num_samples: 0,
            num_inputs: 0,
            num_outputs: 0,
            inputs: std::ptr::null_mut(),
            outputs: std::ptr::null_mut(),
            input_param_changes: std::ptr::null_mut(),
            output_param_changes: std::ptr::null_mut(),
            input_events: std::ptr::null_mut(),
            output_events: std::ptr::null_mut(),
            context: std::ptr::null_mut(),
        }
    }
}

/// Process context (transport)
#[repr(C)]
#[derive(Debug, Clone)]
pub struct ProcessContext {
    pub state: u32,
    pub sample_rate: f64,
    pub project_time_samples: i64,
    pub system_time: i64,
    pub continuous_time_samples: i64,
    pub project_time_music: f64,
    pub bar_position_music: f64,
    pub cycle_start_music: f64,
    pub cycle_end_music: f64,
    pub tempo: f64,
    pub time_sig_numerator: i32,
    pub time_sig_denominator: i32,
    pub chord: i32,
    pub smpte_offset_subframes: i32,
    pub frame_rate: u32,
    pub samples_to_next_clock: i32,
}

impl Default for ProcessContext {
    fn default() -> Self {
        Self {
            state: 0,
            sample_rate: 44100.0,
            project_time_samples: 0,
            system_time: 0,
            continuous_time_samples: 0,
            project_time_music: 0.0,
            bar_position_music: 0.0,
            cycle_start_music: 0.0,
            cycle_end_music: 0.0,
            tempo: 120.0,
            time_sig_numerator: 4,
            time_sig_denominator: 4,
            chord: 0,
            smpte_offset_subframes: 0,
            frame_rate: 0,
            samples_to_next_clock: 0,
        }
    }
}

// Helper functions

fn copy_str_to_cstr(src: &str, dst: &mut [i8]) {
    let bytes = src.as_bytes();
    let len = bytes.len().min(dst.len() - 1);
    for (i, &b) in bytes[..len].iter().enumerate() {
        dst[i] = b as i8;
    }
    dst[len] = 0;
}

fn copy_str_to_u16(src: &str, dst: &mut [u16]) {
    let len = src.chars().count().min(dst.len() - 1);
    for (i, c) in src.chars().take(len).enumerate() {
        dst[i] = c as u16;
    }
    dst[len] = 0;
}

/// Compare two TUIDs for equality
pub fn tuid_eq(a: &TUID, b: &TUID) -> bool {
    a == b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_result_codes() {
        assert_eq!(K_RESULT_OK, 0);
        assert_eq!(K_RESULT_TRUE, 0);
        assert_eq!(K_RESULT_FALSE, 1);
        assert_eq!(K_NOT_INITIALIZED, 5);
    }

    #[test]
    fn test_speaker_arrangements() {
        assert_eq!(K_STEREO, K_SPEAKER_L | K_SPEAKER_R);
        assert_eq!(K_MONO, K_SPEAKER_C);
    }

    #[test]
    fn test_factory_info_new() {
        let info = PFactoryInfo::new("TestVendor", "https://test.com", "test@test.com", K_UNICODE);

        // Check vendor string
        let vendor: Vec<u8> = info.vendor.iter()
            .take_while(|&&c| c != 0)
            .map(|&c| c as u8)
            .collect();
        assert_eq!(String::from_utf8(vendor).unwrap(), "TestVendor");
    }

    #[test]
    fn test_class_info_new() {
        let cid: TUID = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let info = PClassInfo::new(cid, categories::FX, "TestPlugin");

        assert_eq!(info.cid, cid);
        assert_eq!(info.cardinality, K_MANY_INSTANCES);
    }

    #[test]
    fn test_bus_info() {
        let info = BusInfo::audio("Stereo Out", K_OUTPUT, 2, K_MAIN);
        assert_eq!(info.media_type, K_AUDIO);
        assert_eq!(info.direction, K_OUTPUT);
        assert_eq!(info.channel_count, 2);
    }

    #[test]
    fn test_parameter_info() {
        let info = ParameterInfo::new(0, "Volume", 0.5, K_CAN_AUTOMATE)
            .with_unit("dB")
            .with_steps(100);

        assert_eq!(info.id, 0);
        assert_eq!(info.default_normalized_value, 0.5);
        assert_eq!(info.step_count, 100);
    }

    #[test]
    fn test_tuid_eq() {
        let a: TUID = [1; 16];
        let b: TUID = [1; 16];
        let c: TUID = [2; 16];

        assert!(tuid_eq(&a, &b));
        assert!(!tuid_eq(&a, &c));
    }

    #[test]
    fn test_iid_constants() {
        // Verify IID lengths
        assert_eq!(iid::FUNKNOWN.len(), 16);
        assert_eq!(iid::IPLUGIN_FACTORY.len(), 16);
        assert_eq!(iid::ICOMPONENT.len(), 16);
        assert_eq!(iid::IAUDIO_PROCESSOR.len(), 16);
        assert_eq!(iid::IEDIT_CONTROLLER.len(), 16);
    }

    #[test]
    fn test_process_setup_default() {
        let setup = ProcessSetup::default();
        assert_eq!(setup.process_mode, K_REALTIME);
        assert_eq!(setup.symbolic_sample_size, K_SAMPLE_32);
        assert_eq!(setup.sample_rate, 44100.0);
    }

    #[test]
    fn test_struct_sizes() {
        // Verify repr(C) layout sizes are reasonable
        assert!(std::mem::size_of::<TUID>() == 16);
        assert!(std::mem::size_of::<PFactoryInfo>() > 0);
        assert!(std::mem::size_of::<PClassInfo>() > 0);
        assert!(std::mem::size_of::<BusInfo>() > 0);
        assert!(std::mem::size_of::<ParameterInfo>() > 0);
    }
}
