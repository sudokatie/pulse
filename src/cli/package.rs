//! Package CLI - build VST3 bundles from Pulse plugins

use std::path::{Path, PathBuf};

use crate::export::bundle::{BundleBuilder, Platform, ValidationResult};
use crate::plugin::PluginInfo;

/// Result of building a package
#[derive(Debug)]
pub struct PackageResult {
    pub bundle_path: PathBuf,
    pub platform: String,
    pub validation: ValidationResult,
}

impl PackageResult {
    pub fn is_success(&self) -> bool {
        self.validation.structure_valid()
    }
}

/// Build options for packaging
#[derive(Debug, Clone)]
pub struct PackageOptions {
    /// Plugin name
    pub name: String,
    /// Plugin ID (bundle identifier base)
    pub id: String,
    /// Vendor name
    pub vendor: String,
    /// Version string
    pub version: String,
    /// Source binary path
    pub binary: Option<PathBuf>,
    /// Output directory
    pub output_dir: PathBuf,
    /// Target platform (defaults to current)
    pub platform: Option<Platform>,
}

impl Default for PackageOptions {
    fn default() -> Self {
        Self {
            name: "MyPlugin".to_string(),
            id: "com.example.myplugin".to_string(),
            vendor: "My Company".to_string(),
            version: "1.0.0".to_string(),
            binary: None,
            output_dir: PathBuf::from("."),
            platform: None,
        }
    }
}

/// Build a VST3 bundle from the given options
pub fn build_package(options: PackageOptions) -> Result<PackageResult, String> {
    let info = PluginInfo {
        id: options.id.clone(),
        name: options.name.clone(),
        vendor: options.vendor.clone(),
        version: options.version.clone(),
        category: crate::plugin::PluginCategory::Effect,
        inputs: 2,
        outputs: 2,
    };

    let platform = options.platform.unwrap_or_else(Platform::current);

    let mut builder = BundleBuilder::new(info)
        .platform(platform)
        .output_dir(&options.output_dir);

    if let Some(binary) = &options.binary {
        builder = builder.binary(binary);
    }

    let bundle_path = builder.build().map_err(|e| e.to_string())?;

    let validation = builder.validate().map_err(|e| e.to_string())?;

    let platform_str = match platform {
        Platform::MacOS => "macOS",
        Platform::Linux => "Linux",
        Platform::Windows => "Windows",
    };

    Ok(PackageResult {
        bundle_path,
        platform: platform_str.to_string(),
        validation,
    })
}

/// Print package result in a friendly format
pub fn print_result(result: &PackageResult) {
    if result.is_success() {
        println!("Successfully created VST3 bundle:");
        println!("  Path: {}", result.bundle_path.display());
        println!("  Platform: {}", result.platform);
    } else {
        println!("Bundle created with issues:");
        println!("  Path: {}", result.bundle_path.display());
        for error in &result.validation.errors {
            println!("  Warning: {}", error);
        }
    }
}

/// Parse platform from string
pub fn parse_platform(s: &str) -> Option<Platform> {
    match s.to_lowercase().as_str() {
        "macos" | "mac" | "darwin" | "osx" => Some(Platform::MacOS),
        "linux" | "gnu" => Some(Platform::Linux),
        "windows" | "win" | "win32" | "win64" => Some(Platform::Windows),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_platform() {
        assert_eq!(parse_platform("macos"), Some(Platform::MacOS));
        assert_eq!(parse_platform("MacOS"), Some(Platform::MacOS));
        assert_eq!(parse_platform("mac"), Some(Platform::MacOS));
        assert_eq!(parse_platform("linux"), Some(Platform::Linux));
        assert_eq!(parse_platform("windows"), Some(Platform::Windows));
        assert_eq!(parse_platform("win"), Some(Platform::Windows));
        assert_eq!(parse_platform("unknown"), None);
    }

    #[test]
    fn test_package_options_default() {
        let options = PackageOptions::default();
        assert_eq!(options.name, "MyPlugin");
        assert_eq!(options.version, "1.0.0");
        assert!(options.binary.is_none());
    }

    #[test]
    fn test_build_package_basic() {
        let temp_dir = TempDir::new().unwrap();

        let options = PackageOptions {
            name: "Test Plugin".to_string(),
            id: "com.test.plugin".to_string(),
            vendor: "Test Vendor".to_string(),
            version: "1.0.0".to_string(),
            binary: None,
            output_dir: temp_dir.path().to_path_buf(),
            platform: Some(Platform::MacOS),
        };

        let result = build_package(options);
        assert!(result.is_ok());

        let result = result.unwrap();
        assert!(result.bundle_path.exists());
        assert_eq!(result.platform, "macOS");
        assert!(result.validation.structure_valid());
    }

    #[test]
    fn test_build_package_with_binary() {
        let temp_dir = TempDir::new().unwrap();

        // Create a fake binary
        let binary_path = temp_dir.path().join("fakebinary");
        std::fs::write(&binary_path, b"fake binary").unwrap();

        let output_dir = temp_dir.path().join("output");

        let options = PackageOptions {
            name: "Test Plugin".to_string(),
            id: "com.test.plugin".to_string(),
            vendor: "Test Vendor".to_string(),
            version: "1.0.0".to_string(),
            binary: Some(binary_path),
            output_dir,
            platform: Some(Platform::MacOS),
        };

        let result = build_package(options);
        assert!(result.is_ok());

        let result = result.unwrap();
        assert!(result.is_success());
        assert!(result.validation.is_valid());
    }

    #[test]
    fn test_package_result_is_success() {
        let validation = ValidationResult {
            bundle_exists: true,
            contents_exists: true,
            binary_dir_exists: true,
            binary_exists: true,
            plist_exists: true,
            plist_valid: true,
            errors: vec![],
        };

        let result = PackageResult {
            bundle_path: PathBuf::from("/test/bundle.vst3"),
            platform: "macOS".to_string(),
            validation,
        };

        assert!(result.is_success());
    }

    #[test]
    fn test_package_result_partial_success() {
        let validation = ValidationResult {
            bundle_exists: true,
            contents_exists: true,
            binary_dir_exists: true,
            binary_exists: false, // Missing binary
            plist_exists: true,
            plist_valid: true,
            errors: vec!["Binary missing".to_string()],
        };

        let result = PackageResult {
            bundle_path: PathBuf::from("/test/bundle.vst3"),
            platform: "macOS".to_string(),
            validation,
        };

        // Structure is valid even without binary
        assert!(result.is_success());
    }
}
