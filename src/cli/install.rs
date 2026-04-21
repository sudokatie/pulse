//! Plugin install CLI commands

use std::fs;
use std::path::{Path, PathBuf};

use crate::export::{BundleBuilder, Platform, ValidationResult, default_install_path};
use crate::host::{PluginScanner, ScannerConfig, PluginFormat};
use crate::plugin::{PluginCategory, PluginInfo};

/// Result of plugin installation
#[derive(Debug)]
pub struct InstallResult {
    /// Source bundle path
    pub source: PathBuf,
    /// Destination path
    pub destination: PathBuf,
    /// Whether installation succeeded
    pub success: bool,
    /// Whether plugin was discovered by scanner
    pub discovered: bool,
    /// Error message if failed
    pub error: Option<String>,
}

/// Install a VST3 bundle to the system plugin directory
pub fn install_bundle(bundle_path: &Path, target_dir: Option<&Path>) -> InstallResult {
    let dest_dir = target_dir
        .map(PathBuf::from)
        .unwrap_or_else(default_install_path);

    let bundle_name = bundle_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown.vst3".to_string());

    let destination = dest_dir.join(&bundle_name);

    // Validate source bundle exists
    if !bundle_path.exists() {
        return InstallResult {
            source: bundle_path.to_path_buf(),
            destination,
            success: false,
            discovered: false,
            error: Some(format!("Source bundle does not exist: {}", bundle_path.display())),
        };
    }

    // Validate it's a VST3 bundle
    if !bundle_name.ends_with(".vst3") {
        return InstallResult {
            source: bundle_path.to_path_buf(),
            destination,
            success: false,
            discovered: false,
            error: Some("Source is not a VST3 bundle (must end with .vst3)".to_string()),
        };
    }

    // Create destination directory if needed
    if let Err(e) = fs::create_dir_all(&dest_dir) {
        return InstallResult {
            source: bundle_path.to_path_buf(),
            destination,
            success: false,
            discovered: false,
            error: Some(format!("Failed to create destination directory: {}", e)),
        };
    }

    // Remove existing bundle if present
    if destination.exists() {
        if let Err(e) = fs::remove_dir_all(&destination) {
            return InstallResult {
                source: bundle_path.to_path_buf(),
                destination,
                success: false,
                discovered: false,
                error: Some(format!("Failed to remove existing bundle: {}", e)),
            };
        }
    }

    // Copy bundle recursively
    if let Err(e) = copy_dir_recursive(bundle_path, &destination) {
        return InstallResult {
            source: bundle_path.to_path_buf(),
            destination,
            success: false,
            discovered: false,
            error: Some(format!("Failed to copy bundle: {}", e)),
        };
    }

    // Verify installation by scanning
    let mut config = ScannerConfig::default();
    config.search_paths = vec![dest_dir];
    config.formats = vec![PluginFormat::Vst3];

    let scanner = PluginScanner::with_config(config);
    let plugins = scanner.scan();

    let discovered = plugins.iter().any(|p| p.path == destination);

    InstallResult {
        source: bundle_path.to_path_buf(),
        destination,
        success: true,
        discovered,
        error: None,
    }
}

/// Copy a directory recursively
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

/// Validate a VST3 bundle structure
pub fn validate_bundle(bundle_path: &Path) -> ValidationResult {
    // Extract plugin name from bundle
    let name = bundle_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string();

    // Create a minimal PluginInfo for the builder
    let info = PluginInfo {
        id: format!("validate.{}", name.to_lowercase().replace(' ', "-")),
        name,
        vendor: "Unknown".to_string(),
        version: "1.0.0".to_string(),
        category: PluginCategory::Effect,
        inputs: 2,
        outputs: 2,
    };

    // Use BundleBuilder to validate
    let output_dir = bundle_path.parent().unwrap_or(Path::new("."));
    let builder = BundleBuilder::new(info)
        .platform(Platform::current())
        .output_dir(output_dir);

    builder.validate().unwrap_or_else(|_| ValidationResult {
        bundle_exists: false,
        contents_exists: false,
        binary_dir_exists: false,
        binary_exists: false,
        plist_exists: false,
        plist_valid: false,
        errors: vec!["Failed to validate bundle".to_string()],
    })
}

/// Print install result
pub fn print_install_result(result: &InstallResult) {
    if result.success {
        println!("Installed: {} -> {}", result.source.display(), result.destination.display());
        if result.discovered {
            println!("Plugin discovered by scanner");
        } else {
            println!("Warning: Plugin not found by scanner");
        }
    } else {
        eprintln!("Installation failed: {}", result.error.as_deref().unwrap_or("Unknown error"));
    }
}

/// Print validation result
pub fn print_validation_result(path: &Path, result: &ValidationResult) {
    println!("Validating: {}", path.display());
    println!();

    let check = |ok: bool, name: &str| {
        if ok {
            println!("  [OK] {}", name);
        } else {
            println!("  [FAIL] {}", name);
        }
    };

    check(result.bundle_exists, "Bundle directory exists");
    check(result.contents_exists, "Contents directory exists");
    check(result.binary_dir_exists, "Platform binary directory exists");
    check(result.binary_exists, "Binary file exists");
    check(result.plist_exists, "Info.plist exists");
    check(result.plist_valid, "Info.plist is valid");

    println!();

    if result.is_valid() {
        println!("Bundle is valid");
    } else if result.structure_valid() {
        println!("Bundle structure is valid (binary missing)");
    } else {
        println!("Bundle is invalid:");
        for err in &result.errors {
            println!("  - {}", err);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_bundle(temp_dir: &Path, name: &str) -> PathBuf {
        let bundle_path = temp_dir.join(format!("{}.vst3", name));
        let contents = bundle_path.join("Contents");
        let macos = contents.join("MacOS");

        fs::create_dir_all(&macos).unwrap();

        // Create Info.plist
        let plist = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>com.test.plugin</string>
    <key>CFBundleExecutable</key>
    <string>TestPlugin</string>
    <key>CFBundleVersion</key>
    <string>1.0.0</string>
</dict>
</plist>"#;
        fs::write(contents.join("Info.plist"), plist).unwrap();

        // Create fake binary
        fs::write(macos.join(name), b"fake binary").unwrap();

        bundle_path
    }

    #[test]
    fn test_install_bundle() {
        let source_dir = TempDir::new().unwrap();
        let dest_dir = TempDir::new().unwrap();

        let bundle = create_test_bundle(source_dir.path(), "TestPlugin");
        let result = install_bundle(&bundle, Some(dest_dir.path()));

        assert!(result.success);
        assert!(result.destination.exists());
        assert!(result.error.is_none());
    }

    #[test]
    fn test_install_nonexistent_bundle() {
        let dest_dir = TempDir::new().unwrap();
        let fake_path = PathBuf::from("/nonexistent/path/Plugin.vst3");

        let result = install_bundle(&fake_path, Some(dest_dir.path()));

        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_install_non_vst3() {
        let temp_dir = TempDir::new().unwrap();
        let not_vst3 = temp_dir.path().join("NotAPlugin.txt");
        fs::write(&not_vst3, "not a plugin").unwrap();

        let result = install_bundle(&not_vst3, Some(temp_dir.path()));

        assert!(!result.success);
        assert!(result.error.as_ref().unwrap().contains("VST3"));
    }

    #[test]
    fn test_validate_bundle() {
        let temp_dir = TempDir::new().unwrap();
        let bundle = create_test_bundle(temp_dir.path(), "TestPlugin");

        let result = validate_bundle(&bundle);

        assert!(result.bundle_exists);
        assert!(result.contents_exists);
        assert!(result.plist_exists);
        assert!(result.plist_valid);
    }

    #[test]
    fn test_validate_nonexistent_bundle() {
        let fake_path = PathBuf::from("/nonexistent/Plugin.vst3");
        let result = validate_bundle(&fake_path);

        assert!(!result.bundle_exists);
        assert!(!result.is_valid());
    }

    #[test]
    fn test_copy_dir_recursive() {
        let temp_dir = TempDir::new().unwrap();
        let src = temp_dir.path().join("src");
        let dst = temp_dir.path().join("dst");

        // Create nested structure
        fs::create_dir_all(src.join("sub")).unwrap();
        fs::write(src.join("file1.txt"), "content1").unwrap();
        fs::write(src.join("sub/file2.txt"), "content2").unwrap();

        copy_dir_recursive(&src, &dst).unwrap();

        assert!(dst.join("file1.txt").exists());
        assert!(dst.join("sub/file2.txt").exists());
        assert_eq!(fs::read_to_string(dst.join("file1.txt")).unwrap(), "content1");
    }

    #[test]
    fn test_install_overwrites_existing() {
        let source_dir = TempDir::new().unwrap();
        let dest_dir = TempDir::new().unwrap();

        let bundle = create_test_bundle(source_dir.path(), "TestPlugin");

        // First install
        let result1 = install_bundle(&bundle, Some(dest_dir.path()));
        assert!(result1.success);

        // Second install should succeed (overwrite)
        let result2 = install_bundle(&bundle, Some(dest_dir.path()));
        assert!(result2.success);
    }

    #[test]
    fn test_install_result_discovered() {
        let source_dir = TempDir::new().unwrap();
        let dest_dir = TempDir::new().unwrap();

        let bundle = create_test_bundle(source_dir.path(), "TestPlugin");
        let result = install_bundle(&bundle, Some(dest_dir.path()));

        assert!(result.success);
        // Scanner should find it
        assert!(result.discovered);
    }
}
