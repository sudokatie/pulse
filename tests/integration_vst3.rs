//! Integration tests for VST3 plugin export
//!
//! Tests the full round-trip: create plugin -> build bundle -> scan -> verify

use std::fs;
use std::path::PathBuf;

use pulse::buffer::AudioBuffer;
use pulse::export::{BundleBuilder, Platform, default_install_path};
use pulse::host::{PluginScanner, ScannerConfig, PluginFormat};
use pulse::param::ParamInfo;
use pulse::plugin::{Plugin, PluginCategory, PluginConfig, PluginInfo};
use pulse::process::ProcessContext;
use tempfile::TempDir;

/// Test plugin for integration testing
#[derive(Default)]
struct TestIntegrationPlugin {
    gain: f32,
    mix: f32,
}

impl Plugin for TestIntegrationPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            id: "com.pulse.test.integration".to_string(),
            name: "Integration Test Plugin".to_string(),
            vendor: "Pulse Test".to_string(),
            version: "1.0.0".to_string(),
            category: PluginCategory::Effect,
            inputs: 2,
            outputs: 2,
        }
    }

    fn init(&mut self, _config: &PluginConfig) -> pulse::Result<()> {
        self.gain = 1.0;
        self.mix = 0.5;
        Ok(())
    }

    fn process(&mut self, buffer: &mut AudioBuffer, _ctx: &ProcessContext) {
        for ch in 0..buffer.channels() {
            if let Some(channel) = buffer.channel_mut(ch) {
                for sample in channel.iter_mut() {
                    *sample *= self.gain * self.mix;
                }
            }
        }
    }

    fn parameters(&self) -> Vec<ParamInfo> {
        vec![
            ParamInfo::float(0, "Gain", 0.0, 2.0, 1.0),
            ParamInfo::float(1, "Mix", 0.0, 1.0, 0.5),
        ]
    }

    fn set_parameter(&mut self, id: u32, value: f32) {
        match id {
            0 => self.gain = value,
            1 => self.mix = value,
            _ => {}
        }
    }

    fn get_parameter(&self, id: u32) -> f32 {
        match id {
            0 => self.gain,
            1 => self.mix,
            _ => 0.0,
        }
    }

    fn get_state(&self) -> Vec<u8> {
        let mut state = Vec::new();
        state.extend_from_slice(&self.gain.to_le_bytes());
        state.extend_from_slice(&self.mix.to_le_bytes());
        state
    }

    fn set_state(&mut self, data: &[u8]) -> pulse::Result<()> {
        if data.len() >= 8 {
            self.gain = f32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            self.mix = f32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.gain = 1.0;
        self.mix = 0.5;
    }
}

/// Create a test bundle with fake binary content
fn create_test_bundle(temp_dir: &TempDir, info: &PluginInfo) -> PathBuf {
    let builder = BundleBuilder::new(info.clone())
        .platform(Platform::current())
        .output_dir(temp_dir.path());

    // Build the bundle structure
    let bundle_path = builder.build().expect("Failed to build bundle");

    // Create a fake binary file
    let binary_path = builder.binary_dest_path();
    fs::write(&binary_path, b"FAKE_VST3_BINARY_CONTENT").expect("Failed to write binary");

    bundle_path
}

#[test]
fn test_bundle_build_and_validate() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let plugin = TestIntegrationPlugin::default();
    let info = plugin.info();

    let bundle_path = create_test_bundle(&temp_dir, &info);

    // Verify bundle exists
    assert!(bundle_path.exists());
    assert!(bundle_path.is_dir());

    // Verify structure
    let contents = bundle_path.join("Contents");
    assert!(contents.exists());
    assert!(contents.join("Info.plist").exists());

    // Verify binary directory
    let binary_dir = contents.join(Platform::current().arch_dir());
    assert!(binary_dir.exists());
}

#[test]
fn test_bundle_scanner_discovers_plugin() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let plugin = TestIntegrationPlugin::default();
    let info = plugin.info();

    let bundle_path = create_test_bundle(&temp_dir, &info);

    // Configure scanner to search in temp directory
    let mut config = ScannerConfig::default();
    config.search_paths = vec![temp_dir.path().to_path_buf()];
    config.formats = vec![PluginFormat::Vst3];

    let scanner = PluginScanner::with_config(config);
    let plugins = scanner.scan();

    // Should find exactly one plugin
    assert_eq!(plugins.len(), 1);

    // Verify plugin info
    let scanned = &plugins[0];
    assert_eq!(scanned.name, "IntegrationTestPlugin");
    assert_eq!(scanned.format, PluginFormat::Vst3);
    assert_eq!(scanned.path, bundle_path);
}

#[test]
fn test_plugin_info_round_trip() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let plugin = TestIntegrationPlugin::default();
    let info = plugin.info();

    // Create bundle
    let bundle_path = create_test_bundle(&temp_dir, &info);

    // Scan for plugin
    let mut config = ScannerConfig::default();
    config.search_paths = vec![temp_dir.path().to_path_buf()];
    config.formats = vec![PluginFormat::Vst3];

    let scanner = PluginScanner::with_config(config);
    let plugins = scanner.scan();

    assert_eq!(plugins.len(), 1);

    // Verify Info.plist contains correct metadata
    let plist_path = bundle_path.join("Contents").join("Info.plist");
    let plist_content = fs::read_to_string(&plist_path).expect("Failed to read plist");

    assert!(plist_content.contains(&info.name));
    assert!(plist_content.contains(&info.version));
    assert!(plist_content.contains(&info.vendor));
}

#[test]
fn test_bundle_validation() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let plugin = TestIntegrationPlugin::default();
    let info = plugin.info();

    let bundle_path = create_test_bundle(&temp_dir, &info);

    // Validate bundle
    let builder = BundleBuilder::new(info)
        .platform(Platform::current())
        .output_dir(temp_dir.path());

    let validation = builder.validate().expect("Validation failed");

    assert!(validation.bundle_exists);
    assert!(validation.contents_exists);
    assert!(validation.binary_dir_exists);
    assert!(validation.binary_exists);
    assert!(validation.plist_exists);
    assert!(validation.plist_valid);
    assert!(validation.is_valid());
}

#[test]
fn test_multiple_plugins_in_directory() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create multiple plugins
    let plugins_info = vec![
        PluginInfo {
            id: "com.test.plugin1".to_string(),
            name: "Test Plugin One".to_string(),
            vendor: "Test Vendor".to_string(),
            version: "1.0.0".to_string(),
            category: PluginCategory::Effect,
            inputs: 2,
            outputs: 2,
        },
        PluginInfo {
            id: "com.test.plugin2".to_string(),
            name: "Test Plugin Two".to_string(),
            vendor: "Test Vendor".to_string(),
            version: "2.0.0".to_string(),
            category: PluginCategory::Instrument,
            inputs: 0,
            outputs: 2,
        },
    ];

    for info in &plugins_info {
        create_test_bundle(&temp_dir, info);
    }

    // Scan for all plugins
    let mut config = ScannerConfig::default();
    config.search_paths = vec![temp_dir.path().to_path_buf()];
    config.formats = vec![PluginFormat::Vst3];

    let scanner = PluginScanner::with_config(config);
    let plugins = scanner.scan();

    assert_eq!(plugins.len(), 2);

    // Verify both are VST3
    assert!(plugins.iter().all(|p| p.format == PluginFormat::Vst3));

    // Verify names (sorted alphabetically)
    let names: Vec<&str> = plugins.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"TestPluginOne"));
    assert!(names.contains(&"TestPluginTwo"));
}

#[test]
fn test_plugin_parameters() {
    let mut plugin = TestIntegrationPlugin::default();
    plugin.init(&PluginConfig::default()).unwrap();

    let params = plugin.parameters();
    assert_eq!(params.len(), 2);

    // Verify parameter details
    assert_eq!(params[0].name, "Gain");
    assert_eq!(params[1].name, "Mix");

    // Test set/get
    plugin.set_parameter(0, 0.75);
    assert!((plugin.get_parameter(0) - 0.75).abs() < 0.001);

    plugin.set_parameter(1, 0.25);
    assert!((plugin.get_parameter(1) - 0.25).abs() < 0.001);
}

#[test]
fn test_plugin_state_persistence() {
    let mut plugin = TestIntegrationPlugin::default();
    plugin.init(&PluginConfig::default()).unwrap();

    // Set custom values
    plugin.set_parameter(0, 1.5);
    plugin.set_parameter(1, 0.75);

    // Save state
    let state = plugin.get_state();
    assert_eq!(state.len(), 8); // 2 f32 values

    // Create new plugin and restore state
    let mut plugin2 = TestIntegrationPlugin::default();
    plugin2.set_state(&state).unwrap();

    assert!((plugin2.get_parameter(0) - 1.5).abs() < 0.001);
    assert!((plugin2.get_parameter(1) - 0.75).abs() < 0.001);
}

#[test]
fn test_default_install_path_valid() {
    let path = default_install_path();

    // Should return a non-empty path
    assert!(!path.as_os_str().is_empty());

    // Should contain VST3 or vst3 in the path
    let path_str = path.to_string_lossy().to_lowercase();
    assert!(path_str.contains("vst3"));
}

#[test]
fn test_platform_detection() {
    let platform = Platform::current();

    // Verify platform is valid
    assert!(matches!(
        platform,
        Platform::MacOS | Platform::Linux | Platform::Windows
    ));

    // Verify arch directory is set
    let arch_dir = platform.arch_dir();
    assert!(!arch_dir.is_empty());
}

#[test]
fn test_bundle_name_formatting() {
    let info = PluginInfo {
        id: "com.test.my-awesome-plugin".to_string(),
        name: "My Awesome Plugin".to_string(),
        vendor: "Test".to_string(),
        version: "1.0.0".to_string(),
        category: PluginCategory::Effect,
        inputs: 2,
        outputs: 2,
    };

    let builder = BundleBuilder::new(info);

    // Spaces should be removed from bundle name
    assert_eq!(builder.bundle_name(), "MyAwesomePlugin.vst3");
}

#[test]
fn test_process_audio() {
    let mut plugin = TestIntegrationPlugin::default();
    plugin.init(&PluginConfig::default()).unwrap();

    // Set known values
    plugin.set_parameter(0, 2.0); // gain
    plugin.set_parameter(1, 0.5); // mix

    // Create test buffer
    let mut buffer = AudioBuffer::from_interleaved(&[1.0, 1.0, 1.0, 1.0], 2);

    let ctx = ProcessContext::default();
    plugin.process(&mut buffer, &ctx);

    // Expected: 1.0 * 2.0 * 0.5 = 1.0
    for ch in 0..buffer.channels() {
        if let Some(channel) = buffer.channel(ch) {
            for &sample in channel {
                assert!((sample - 1.0).abs() < 0.001);
            }
        }
    }
}

#[test]
fn test_scan_specific_path() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let plugin = TestIntegrationPlugin::default();
    let info = plugin.info();

    let bundle_path = create_test_bundle(&temp_dir, &info);

    // Use scan_path to scan a specific bundle
    let scanner = PluginScanner::new();
    let plugins = scanner.scan_path(&bundle_path);

    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].path, bundle_path);
}

#[test]
fn test_plugin_reset() {
    let mut plugin = TestIntegrationPlugin::default();
    plugin.init(&PluginConfig::default()).unwrap();

    // Modify values
    plugin.set_parameter(0, 1.8);
    plugin.set_parameter(1, 0.9);

    // Reset
    plugin.reset();

    // Should be back to defaults
    assert!((plugin.get_parameter(0) - 1.0).abs() < 0.001);
    assert!((plugin.get_parameter(1) - 0.5).abs() < 0.001);
}
