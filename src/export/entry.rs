//! VST3 plugin entry point

use std::ffi::c_void;
use std::sync::OnceLock;

use crate::plugin::{Plugin, PluginInfo};

use super::factory::{FactoryInfo, Vst3PluginFactory, generate_tuid};

/// Plugin creation function type
pub type PluginCreateFn = fn() -> Box<dyn Plugin>;

/// Global plugin registry for entry point
static PLUGIN_REGISTRY: OnceLock<PluginRegistry> = OnceLock::new();

/// Registry holding plugin creation info
pub struct PluginRegistry {
    factory_info: FactoryInfo,
    plugins: Vec<RegisteredPlugin>,
}

struct RegisteredPlugin {
    create: PluginCreateFn,
}

impl PluginRegistry {
    /// Create a new registry
    pub fn new(info: FactoryInfo) -> Self {
        Self {
            factory_info: info,
            plugins: Vec::new(),
        }
    }

    /// Register a plugin
    pub fn register(&mut self, create: PluginCreateFn) {
        self.plugins.push(RegisteredPlugin { create });
    }

    /// Build the factory from registered plugins
    pub fn build_factory(&self) -> Box<Vst3PluginFactory> {
        let mut factory = Vst3PluginFactory::new(self.factory_info.clone());

        for registered in &self.plugins {
            let plugin = (registered.create)();
            let info = plugin.info();
            drop(plugin);

            let processor_cid = generate_tuid(&info.id, "processor");
            let controller_cid = generate_tuid(&info.id, "controller");

            let create_fn = registered.create;

            factory.register_processor(
                processor_cid,
                controller_cid,
                &info,
                move || create_fn(),
            );

            factory.register_controller(
                controller_cid,
                &info,
                move || create_fn(),
            );
        }

        factory
    }
}

/// Initialize the plugin registry
pub fn init_registry(info: FactoryInfo) -> &'static PluginRegistry {
    PLUGIN_REGISTRY.get_or_init(|| PluginRegistry::new(info))
}

/// Register a plugin with the global registry
pub fn register_plugin(create: PluginCreateFn) {
    // Note: This requires init_registry to be called first
    // In practice, plugins use the macro which handles this
}

/// Create the plugin factory from the registry
pub fn create_factory() -> Option<Box<Vst3PluginFactory>> {
    PLUGIN_REGISTRY.get().map(|r| r.build_factory())
}

/// VST3 entry point function type
pub type GetPluginFactoryFn = unsafe extern "C" fn() -> *mut c_void;

/// VST3 module init function type (Windows)
#[cfg(target_os = "windows")]
pub type InitDllFn = unsafe extern "system" fn() -> bool;

/// VST3 module exit function type (Windows)
#[cfg(target_os = "windows")]
pub type ExitDllFn = unsafe extern "system" fn() -> bool;

/// VST3 module init function type (macOS/Linux)
#[cfg(not(target_os = "windows"))]
pub type ModuleEntryFn = unsafe extern "C" fn(*mut c_void) -> bool;

/// VST3 module exit function type (macOS/Linux)
#[cfg(not(target_os = "windows"))]
pub type ModuleExitFn = unsafe extern "C" fn() -> bool;

/// Macro to create VST3 entry point
///
/// Usage:
/// ```ignore
/// pulse::export::vst3_entry_point!(MyPlugin, "My Vendor", "https://example.com", "info@example.com");
/// ```
#[macro_export]
macro_rules! vst3_entry_point {
    ($plugin:ty, $vendor:expr, $url:expr, $email:expr) => {
        #[no_mangle]
        pub unsafe extern "C" fn GetPluginFactory() -> *mut std::ffi::c_void {
            use $crate::export::factory::{FactoryInfo, Vst3PluginFactory, generate_tuid};
            use $crate::plugin::Plugin;

            let info = FactoryInfo {
                vendor: $vendor.to_string(),
                url: $url.to_string(),
                email: $email.to_string(),
            };

            let mut factory = Vst3PluginFactory::new(info);

            let create_plugin = || -> Box<dyn Plugin> {
                Box::new(<$plugin>::default())
            };

            let plugin = create_plugin();
            let plugin_info = plugin.info();
            drop(plugin);

            let processor_cid = generate_tuid(&plugin_info.id, "processor");
            let controller_cid = generate_tuid(&plugin_info.id, "controller");

            factory.register_processor(
                processor_cid,
                controller_cid,
                &plugin_info,
                move || Box::new(<$plugin>::default()),
            );

            factory.register_controller(
                controller_cid,
                &plugin_info,
                move || Box::new(<$plugin>::default()),
            );

            Box::into_raw(factory) as *mut std::ffi::c_void
        }

        #[cfg(target_os = "windows")]
        #[no_mangle]
        pub unsafe extern "system" fn InitDll() -> bool {
            true
        }

        #[cfg(target_os = "windows")]
        #[no_mangle]
        pub unsafe extern "system" fn ExitDll() -> bool {
            true
        }

        #[cfg(not(target_os = "windows"))]
        #[no_mangle]
        pub unsafe extern "C" fn ModuleEntry(_: *mut std::ffi::c_void) -> bool {
            true
        }

        #[cfg(not(target_os = "windows"))]
        #[no_mangle]
        pub unsafe extern "C" fn ModuleExit() -> bool {
            true
        }
    };
}

/// Helper to manually create an entry point for testing
pub fn create_entry_point_factory<P: Plugin + Default + 'static>(
    vendor: &str,
    url: &str,
    email: &str,
) -> *mut c_void {
    use super::factory::{FactoryInfo, Vst3PluginFactory, generate_tuid};

    let info = FactoryInfo {
        vendor: vendor.to_string(),
        url: url.to_string(),
        email: email.to_string(),
    };

    let mut factory = Vst3PluginFactory::new(info);

    let create_plugin = || -> Box<dyn Plugin> {
        Box::new(P::default())
    };

    let plugin = create_plugin();
    let plugin_info = plugin.info();
    drop(plugin);

    let processor_cid = generate_tuid(&plugin_info.id, "processor");
    let controller_cid = generate_tuid(&plugin_info.id, "controller");

    factory.register_processor(
        processor_cid,
        controller_cid,
        &plugin_info,
        move || Box::new(P::default()),
    );

    factory.register_controller(
        controller_cid,
        &plugin_info,
        move || Box::new(P::default()),
    );

    Box::into_raw(factory) as *mut c_void
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::AudioBuffer;
    use crate::param::ParamInfo;
    use crate::plugin::{PluginCategory, PluginConfig};
    use crate::process::ProcessContext;

    #[derive(Default)]
    struct TestEntryPlugin;

    impl Plugin for TestEntryPlugin {
        fn info(&self) -> PluginInfo {
            PluginInfo {
                id: "test.entry.plugin".to_string(),
                name: "Test Entry Plugin".to_string(),
                vendor: "Test Vendor".to_string(),
                version: "1.0.0".to_string(),
                category: PluginCategory::Effect,
                inputs: 2,
                outputs: 2,
            }
        }

        fn init(&mut self, _config: &PluginConfig) -> crate::Result<()> {
            Ok(())
        }

        fn process(&mut self, _buffer: &mut AudioBuffer, _ctx: &ProcessContext) {}

        fn parameters(&self) -> Vec<ParamInfo> {
            vec![ParamInfo::float(0, "Gain", 0.0, 1.0, 0.5)]
        }

        fn set_parameter(&mut self, _id: u32, _value: f32) {}
        fn get_parameter(&self, _id: u32) -> f32 { 0.0 }
        fn get_state(&self) -> Vec<u8> { vec![] }
        fn set_state(&mut self, _data: &[u8]) -> crate::Result<()> { Ok(()) }
        fn reset(&mut self) {}
    }

    #[test]
    fn test_create_entry_point_factory() {
        let factory_ptr = create_entry_point_factory::<TestEntryPlugin>(
            "Test Vendor",
            "https://test.com",
            "test@test.com",
        );

        assert!(!factory_ptr.is_null());

        unsafe {
            let factory = factory_ptr as *mut Vst3PluginFactory;

            // Should have 2 classes: processor and controller
            assert_eq!((*factory).class_count(), 2);

            // Clean up
            drop(Box::from_raw(factory));
        }
    }

    #[test]
    fn test_entry_point_factory_creates_instances() {
        use super::super::types::{iid, K_RESULT_OK};
        use super::super::factory::factory_create_instance;
        use super::super::factory::generate_tuid;
        use super::super::factory::IPluginFactory3;

        let factory_ptr = create_entry_point_factory::<TestEntryPlugin>(
            "Test Vendor",
            "https://test.com",
            "test@test.com",
        );

        let factory = factory_ptr as *mut IPluginFactory3;

        unsafe {
            let processor_cid = generate_tuid("test.entry.plugin", "processor");
            let mut obj: *mut c_void = std::ptr::null_mut();

            let result = factory_create_instance(
                factory,
                &processor_cid,
                &iid::ICOMPONENT,
                &mut obj,
            );

            assert_eq!(result, K_RESULT_OK);
            assert!(!obj.is_null());

            // Clean up instance
            use super::super::adapter::Vst3Adapter;
            let adapter = obj as *mut Vst3Adapter;
            (*adapter).com.release();
            drop(Box::from_raw(adapter));

            // Clean up factory
            drop(Box::from_raw(factory as *mut Vst3PluginFactory));
        }
    }

    #[test]
    fn test_plugin_registry() {
        let info = FactoryInfo {
            vendor: "Test".to_string(),
            url: "https://test.com".to_string(),
            email: "test@test.com".to_string(),
        };

        let mut registry = PluginRegistry::new(info);

        fn create_test() -> Box<dyn Plugin> {
            Box::new(TestEntryPlugin::default())
        }

        registry.register(create_test);

        let factory = registry.build_factory();
        assert_eq!(factory.class_count(), 2); // processor + controller
    }

    #[test]
    fn test_generate_tuid_consistency() {
        let tuid1 = generate_tuid("com.example.myplugin", "processor");
        let tuid2 = generate_tuid("com.example.myplugin", "processor");

        assert_eq!(tuid1, tuid2);
    }

    #[test]
    fn test_entry_point_class_names() {
        let factory_ptr = create_entry_point_factory::<TestEntryPlugin>(
            "Test Vendor",
            "https://test.com",
            "test@test.com",
        );

        unsafe {
            let factory = factory_ptr as *mut Vst3PluginFactory;

            let class0 = (*factory).get_class(0).unwrap();
            let class1 = (*factory).get_class(1).unwrap();

            // One should be processor, one should be controller
            let names: Vec<&str> = vec![&class0.name, &class1.name];
            assert!(names.iter().any(|n| n.contains("Test Entry Plugin")));
            assert!(names.iter().any(|n| n.contains("Controller")));

            drop(Box::from_raw(factory));
        }
    }
}
