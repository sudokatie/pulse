//! VST3 bundle builder - creates platform-specific VST3 bundles

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::plugin::PluginInfo;

/// Platform target for bundle building
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    MacOS,
    Linux,
    Windows,
}

impl Platform {
    /// Detect current platform
    pub fn current() -> Self {
        #[cfg(target_os = "macos")]
        return Platform::MacOS;

        #[cfg(target_os = "linux")]
        return Platform::Linux;

        #[cfg(target_os = "windows")]
        return Platform::Windows;

        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        return Platform::Linux; // Default fallback
    }

    /// Get the architecture subdirectory name
    pub fn arch_dir(&self) -> &'static str {
        match self {
            Platform::MacOS => "MacOS",
            Platform::Linux => {
                #[cfg(target_arch = "x86_64")]
                return "x86_64-linux";
                #[cfg(target_arch = "aarch64")]
                return "aarch64-linux";
                #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
                return "x86_64-linux";
            }
            Platform::Windows => {
                #[cfg(target_arch = "x86_64")]
                return "x86_64-win";
                #[cfg(target_arch = "x86")]
                return "x86-win";
                #[cfg(not(any(target_arch = "x86_64", target_arch = "x86")))]
                return "x86_64-win";
            }
        }
    }

    /// Get the binary extension
    pub fn binary_extension(&self) -> &'static str {
        match self {
            Platform::MacOS => "",
            Platform::Linux => ".so",
            Platform::Windows => ".vst3",
        }
    }
}

/// Bundle builder for creating VST3 plugin bundles
pub struct BundleBuilder {
    /// Plugin info
    info: PluginInfo,
    /// Target platform
    platform: Platform,
    /// Output directory
    output_dir: PathBuf,
    /// Source binary path
    binary_path: Option<PathBuf>,
}

impl BundleBuilder {
    /// Create a new bundle builder
    pub fn new(info: PluginInfo) -> Self {
        Self {
            info,
            platform: Platform::current(),
            output_dir: PathBuf::from("."),
            binary_path: None,
        }
    }

    /// Set the target platform
    pub fn platform(mut self, platform: Platform) -> Self {
        self.platform = platform;
        self
    }

    /// Set the output directory
    pub fn output_dir<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.output_dir = path.as_ref().to_path_buf();
        self
    }

    /// Set the source binary path
    pub fn binary<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.binary_path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Get the bundle name (e.g., "MyPlugin.vst3")
    pub fn bundle_name(&self) -> String {
        let name = self.info.name.replace(' ', "");
        format!("{}.vst3", name)
    }

    /// Get the full bundle path
    pub fn bundle_path(&self) -> PathBuf {
        self.output_dir.join(self.bundle_name())
    }

    /// Get the Contents directory path
    pub fn contents_path(&self) -> PathBuf {
        self.bundle_path().join("Contents")
    }

    /// Get the platform-specific binary directory path
    pub fn binary_dir(&self) -> PathBuf {
        self.contents_path().join(self.platform.arch_dir())
    }

    /// Get the binary filename
    pub fn binary_filename(&self) -> String {
        let name = self.info.name.replace(' ', "");
        format!("{}{}", name, self.platform.binary_extension())
    }

    /// Get the full binary destination path
    pub fn binary_dest_path(&self) -> PathBuf {
        self.binary_dir().join(self.binary_filename())
    }

    /// Build the bundle
    pub fn build(&self) -> crate::Result<PathBuf> {
        let bundle_path = self.bundle_path();

        // Create directory structure
        fs::create_dir_all(self.binary_dir())?;

        // Generate and write Info.plist
        let plist_path = self.contents_path().join("Info.plist");
        let plist_content = self.generate_info_plist();
        let mut file = File::create(&plist_path)?;
        file.write_all(plist_content.as_bytes())?;

        // Copy binary if provided
        if let Some(src) = &self.binary_path {
            if src.exists() {
                fs::copy(src, self.binary_dest_path())?;
            }
        }

        Ok(bundle_path)
    }

    /// Generate Info.plist content
    pub fn generate_info_plist(&self) -> String {
        let bundle_id = format!("com.pulse.vst3.{}", self.info.id.replace('.', "-"));
        let name = &self.info.name;
        let version = &self.info.version;
        let vendor = &self.info.vendor;

        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>English</string>
    <key>CFBundleExecutable</key>
    <string>{}</string>
    <key>CFBundleGetInfoString</key>
    <string>{} {} {}</string>
    <key>CFBundleIdentifier</key>
    <string>{}</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>{}</string>
    <key>CFBundlePackageType</key>
    <string>BNDL</string>
    <key>CFBundleShortVersionString</key>
    <string>{}</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>CFBundleVersion</key>
    <string>{}</string>
</dict>
</plist>
"#,
            self.binary_filename(),
            name,
            version,
            vendor,
            bundle_id,
            name,
            version,
            version,
        )
    }

    /// Validate an existing bundle structure
    pub fn validate(&self) -> Result<ValidationResult, crate::Error> {
        let bundle_path = self.bundle_path();

        let mut result = ValidationResult {
            bundle_exists: false,
            contents_exists: false,
            binary_dir_exists: false,
            binary_exists: false,
            plist_exists: false,
            plist_valid: false,
            errors: Vec::new(),
        };

        // Check bundle directory
        result.bundle_exists = bundle_path.is_dir();
        if !result.bundle_exists {
            result.errors.push("Bundle directory does not exist".to_string());
            return Ok(result);
        }

        // Check Contents directory
        let contents = self.contents_path();
        result.contents_exists = contents.is_dir();
        if !result.contents_exists {
            result.errors.push("Contents directory does not exist".to_string());
        }

        // Check binary directory
        let binary_dir = self.binary_dir();
        result.binary_dir_exists = binary_dir.is_dir();
        if !result.binary_dir_exists {
            result.errors.push(format!(
                "Binary directory {} does not exist",
                self.platform.arch_dir()
            ));
        }

        // Check binary
        let binary_path = self.binary_dest_path();
        result.binary_exists = binary_path.is_file();
        if !result.binary_exists {
            result.errors.push("Binary file does not exist".to_string());
        }

        // Check Info.plist
        let plist_path = contents.join("Info.plist");
        result.plist_exists = plist_path.is_file();
        if result.plist_exists {
            // Basic validation - check if it contains expected keys
            if let Ok(content) = fs::read_to_string(&plist_path) {
                result.plist_valid = content.contains("CFBundleIdentifier")
                    && content.contains("CFBundleExecutable")
                    && content.contains("CFBundleVersion");
                if !result.plist_valid {
                    result.errors.push("Info.plist is missing required keys".to_string());
                }
            } else {
                result.errors.push("Failed to read Info.plist".to_string());
            }
        } else {
            result.errors.push("Info.plist does not exist".to_string());
        }

        Ok(result)
    }
}

/// Result of bundle validation
#[derive(Debug)]
pub struct ValidationResult {
    pub bundle_exists: bool,
    pub contents_exists: bool,
    pub binary_dir_exists: bool,
    pub binary_exists: bool,
    pub plist_exists: bool,
    pub plist_valid: bool,
    pub errors: Vec<String>,
}

impl ValidationResult {
    /// Check if the bundle is fully valid
    pub fn is_valid(&self) -> bool {
        self.bundle_exists
            && self.contents_exists
            && self.binary_dir_exists
            && self.binary_exists
            && self.plist_exists
            && self.plist_valid
    }

    /// Check if the structure is valid (ignoring binary)
    pub fn structure_valid(&self) -> bool {
        self.bundle_exists
            && self.contents_exists
            && self.binary_dir_exists
            && self.plist_exists
            && self.plist_valid
    }
}

/// Get the default VST3 install path for the current platform
pub fn default_install_path() -> PathBuf {
    match Platform::current() {
        Platform::MacOS => {
            if let Some(home) = dirs::home_dir() {
                home.join("Library/Audio/Plug-Ins/VST3")
            } else {
                PathBuf::from("/Library/Audio/Plug-Ins/VST3")
            }
        }
        Platform::Linux => {
            if let Some(home) = dirs::home_dir() {
                home.join(".vst3")
            } else {
                PathBuf::from("/usr/lib/vst3")
            }
        }
        Platform::Windows => {
            if let Some(program_files) = std::env::var_os("ProgramFiles") {
                PathBuf::from(program_files).join("Common Files/VST3")
            } else {
                PathBuf::from("C:/Program Files/Common Files/VST3")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::PluginCategory;
    use tempfile::TempDir;

    fn test_plugin_info() -> PluginInfo {
        PluginInfo {
            id: "com.test.myplugin".to_string(),
            name: "My Test Plugin".to_string(),
            vendor: "Test Vendor".to_string(),
            version: "1.2.3".to_string(),
            category: PluginCategory::Effect,
            inputs: 2,
            outputs: 2,
        }
    }

    #[test]
    fn test_bundle_name() {
        let info = test_plugin_info();
        let builder = BundleBuilder::new(info);

        assert_eq!(builder.bundle_name(), "MyTestPlugin.vst3");
    }

    #[test]
    fn test_platform_arch_dir_macos() {
        assert_eq!(Platform::MacOS.arch_dir(), "MacOS");
    }

    #[test]
    fn test_platform_arch_dir_linux() {
        let dir = Platform::Linux.arch_dir();
        assert!(dir.ends_with("-linux"));
    }

    #[test]
    fn test_platform_arch_dir_windows() {
        let dir = Platform::Windows.arch_dir();
        assert!(dir.ends_with("-win"));
    }

    #[test]
    fn test_platform_binary_extension() {
        assert_eq!(Platform::MacOS.binary_extension(), "");
        assert_eq!(Platform::Linux.binary_extension(), ".so");
        assert_eq!(Platform::Windows.binary_extension(), ".vst3");
    }

    #[test]
    fn test_bundle_paths_macos() {
        let info = test_plugin_info();
        let builder = BundleBuilder::new(info)
            .platform(Platform::MacOS)
            .output_dir("/output");

        assert_eq!(builder.bundle_path(), PathBuf::from("/output/MyTestPlugin.vst3"));
        assert_eq!(builder.contents_path(), PathBuf::from("/output/MyTestPlugin.vst3/Contents"));
        assert_eq!(builder.binary_dir(), PathBuf::from("/output/MyTestPlugin.vst3/Contents/MacOS"));
        assert_eq!(builder.binary_filename(), "MyTestPlugin");
    }

    #[test]
    fn test_bundle_paths_linux() {
        let info = test_plugin_info();
        let builder = BundleBuilder::new(info)
            .platform(Platform::Linux)
            .output_dir("/output");

        let binary_dir = builder.binary_dir();
        assert!(binary_dir.to_str().unwrap().contains("-linux"));
        assert_eq!(builder.binary_filename(), "MyTestPlugin.so");
    }

    #[test]
    fn test_bundle_paths_windows() {
        let info = test_plugin_info();
        let builder = BundleBuilder::new(info)
            .platform(Platform::Windows)
            .output_dir("/output");

        let binary_dir = builder.binary_dir();
        assert!(binary_dir.to_str().unwrap().contains("-win"));
        assert_eq!(builder.binary_filename(), "MyTestPlugin.vst3");
    }

    #[test]
    fn test_generate_info_plist() {
        let info = test_plugin_info();
        let builder = BundleBuilder::new(info).platform(Platform::MacOS);

        let plist = builder.generate_info_plist();

        assert!(plist.contains("CFBundleIdentifier"));
        assert!(plist.contains("CFBundleExecutable"));
        assert!(plist.contains("CFBundleVersion"));
        assert!(plist.contains("My Test Plugin"));
        assert!(plist.contains("1.2.3"));
        assert!(plist.contains("Test Vendor"));
        assert!(plist.contains("com.pulse.vst3.com-test-myplugin"));
        assert!(plist.contains("MyTestPlugin"));
    }

    #[test]
    fn test_build_bundle() {
        let temp_dir = TempDir::new().unwrap();
        let info = test_plugin_info();

        let builder = BundleBuilder::new(info)
            .platform(Platform::MacOS)
            .output_dir(temp_dir.path());

        let result = builder.build();
        assert!(result.is_ok());

        let bundle_path = result.unwrap();
        assert!(bundle_path.is_dir());

        // Check structure
        let contents = bundle_path.join("Contents");
        assert!(contents.is_dir());
        assert!(contents.join("MacOS").is_dir());
        assert!(contents.join("Info.plist").is_file());

        // Check plist content
        let plist_content = fs::read_to_string(contents.join("Info.plist")).unwrap();
        assert!(plist_content.contains("CFBundleIdentifier"));
    }

    #[test]
    fn test_build_bundle_with_binary() {
        let temp_dir = TempDir::new().unwrap();
        let info = test_plugin_info();

        // Create a fake binary
        let fake_binary = temp_dir.path().join("fakebinary");
        fs::write(&fake_binary, b"fake binary content").unwrap();

        let output_dir = temp_dir.path().join("output");

        let builder = BundleBuilder::new(info)
            .platform(Platform::MacOS)
            .output_dir(&output_dir)
            .binary(&fake_binary);

        let result = builder.build();
        assert!(result.is_ok());

        // Check binary was copied
        let binary_path = output_dir
            .join("MyTestPlugin.vst3")
            .join("Contents")
            .join("MacOS")
            .join("MyTestPlugin");
        assert!(binary_path.is_file());
    }

    #[test]
    fn test_validate_bundle() {
        let temp_dir = TempDir::new().unwrap();
        let info = test_plugin_info();

        // Create a fake binary
        let fake_binary = temp_dir.path().join("fakebinary");
        fs::write(&fake_binary, b"fake binary content").unwrap();

        let output_dir = temp_dir.path().join("output");

        let builder = BundleBuilder::new(info)
            .platform(Platform::MacOS)
            .output_dir(&output_dir)
            .binary(&fake_binary);

        builder.build().unwrap();

        let validation = builder.validate().unwrap();
        assert!(validation.bundle_exists);
        assert!(validation.contents_exists);
        assert!(validation.binary_dir_exists);
        assert!(validation.binary_exists);
        assert!(validation.plist_exists);
        assert!(validation.plist_valid);
        assert!(validation.is_valid());
        assert!(validation.errors.is_empty());
    }

    #[test]
    fn test_validate_nonexistent_bundle() {
        let info = test_plugin_info();

        let builder = BundleBuilder::new(info)
            .platform(Platform::MacOS)
            .output_dir("/nonexistent/path");

        let validation = builder.validate().unwrap();
        assert!(!validation.bundle_exists);
        assert!(!validation.is_valid());
        assert!(!validation.errors.is_empty());
    }

    #[test]
    fn test_validate_structure_without_binary() {
        let temp_dir = TempDir::new().unwrap();
        let info = test_plugin_info();

        let builder = BundleBuilder::new(info)
            .platform(Platform::MacOS)
            .output_dir(temp_dir.path());

        // Build without binary
        builder.build().unwrap();

        let validation = builder.validate().unwrap();
        assert!(validation.structure_valid());
        assert!(!validation.is_valid()); // Binary missing
    }

    #[test]
    fn test_validation_result_is_valid() {
        let result = ValidationResult {
            bundle_exists: true,
            contents_exists: true,
            binary_dir_exists: true,
            binary_exists: true,
            plist_exists: true,
            plist_valid: true,
            errors: vec![],
        };
        assert!(result.is_valid());

        let invalid_result = ValidationResult {
            bundle_exists: true,
            contents_exists: true,
            binary_dir_exists: true,
            binary_exists: false,
            plist_exists: true,
            plist_valid: true,
            errors: vec!["Binary missing".to_string()],
        };
        assert!(!invalid_result.is_valid());
    }

    #[test]
    fn test_platform_current() {
        let platform = Platform::current();
        // Just verify it returns a valid platform
        assert!(matches!(
            platform,
            Platform::MacOS | Platform::Linux | Platform::Windows
        ));
    }

    #[test]
    fn test_default_install_path() {
        let path = default_install_path();
        // Just verify it returns a non-empty path
        assert!(!path.as_os_str().is_empty());

        // Platform-specific checks
        match Platform::current() {
            Platform::MacOS => {
                assert!(path.to_str().unwrap().contains("VST3"));
            }
            Platform::Linux => {
                assert!(path.to_str().unwrap().contains("vst3"));
            }
            Platform::Windows => {
                assert!(path.to_str().unwrap().contains("VST3"));
            }
        }
    }

    #[test]
    fn test_plist_xml_structure() {
        let info = test_plugin_info();
        let builder = BundleBuilder::new(info).platform(Platform::MacOS);

        let plist = builder.generate_info_plist();

        // Check XML structure
        assert!(plist.starts_with("<?xml version=\"1.0\""));
        assert!(plist.contains("<!DOCTYPE plist"));
        assert!(plist.contains("<plist version=\"1.0\">"));
        assert!(plist.contains("<dict>"));
        assert!(plist.contains("</dict>"));
        assert!(plist.contains("</plist>"));
    }

    #[test]
    fn test_bundle_builder_chaining() {
        let info = test_plugin_info();

        let builder = BundleBuilder::new(info.clone())
            .platform(Platform::Linux)
            .output_dir("/some/path")
            .binary("/some/binary");

        assert_eq!(builder.platform, Platform::Linux);
        assert_eq!(builder.output_dir, PathBuf::from("/some/path"));
        assert_eq!(builder.binary_path, Some(PathBuf::from("/some/binary")));
    }

    #[test]
    fn test_bundle_name_with_spaces() {
        let mut info = test_plugin_info();
        info.name = "My Cool Plugin Name".to_string();

        let builder = BundleBuilder::new(info);
        assert_eq!(builder.bundle_name(), "MyCoolPluginName.vst3");
    }
}
