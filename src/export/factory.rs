//! VST3 IPluginFactory interface definitions and implementation

use std::ffi::c_void;
use std::sync::atomic::Ordering;

use crate::plugin::{Plugin, PluginInfo};

use super::adapter::Vst3Adapter;
use super::category::category_to_vst3;
use super::com::{ComObject, IUnknownVtable};
use super::controller::Vst3EditController;
use super::types::{
    iid, tuid_eq, PClassInfo, PClassInfo2, PClassInfoW, PFactoryInfo, TResult, TUID,
    K_INVALID_ARGUMENT, K_MANY_INSTANCES, K_NOT_IMPLEMENTED, K_RESULT_OK, K_UNICODE,
};

/// IPluginFactory interface
#[repr(C)]
pub struct IPluginFactory {
    pub vtable: *const IPluginFactoryVtable,
}

/// IPluginFactory vtable
#[repr(C)]
#[derive(Clone, Copy)]
pub struct IPluginFactoryVtable {
    // IUnknown
    pub unknown: IUnknownVtable,
    // IPluginFactory
    pub get_factory_info: unsafe extern "system" fn(
        this: *mut IPluginFactory,
        info: *mut PFactoryInfo,
    ) -> TResult,
    pub count_classes: unsafe extern "system" fn(
        this: *mut IPluginFactory,
    ) -> i32,
    pub get_class_info: unsafe extern "system" fn(
        this: *mut IPluginFactory,
        index: i32,
        info: *mut PClassInfo,
    ) -> TResult,
    pub create_instance: unsafe extern "system" fn(
        this: *mut IPluginFactory,
        cid: *const TUID,
        iid: *const TUID,
        obj: *mut *mut c_void,
    ) -> TResult,
}

/// IPluginFactory2 interface - extends IPluginFactory with extended class info
#[repr(C)]
pub struct IPluginFactory2 {
    pub vtable: *const IPluginFactory2Vtable,
}

/// IPluginFactory2 vtable
#[repr(C)]
#[derive(Clone, Copy)]
pub struct IPluginFactory2Vtable {
    // IUnknown
    pub unknown: IUnknownVtable,
    // IPluginFactory
    pub get_factory_info: unsafe extern "system" fn(
        this: *mut IPluginFactory2,
        info: *mut PFactoryInfo,
    ) -> TResult,
    pub count_classes: unsafe extern "system" fn(
        this: *mut IPluginFactory2,
    ) -> i32,
    pub get_class_info: unsafe extern "system" fn(
        this: *mut IPluginFactory2,
        index: i32,
        info: *mut PClassInfo,
    ) -> TResult,
    pub create_instance: unsafe extern "system" fn(
        this: *mut IPluginFactory2,
        cid: *const TUID,
        iid: *const TUID,
        obj: *mut *mut c_void,
    ) -> TResult,
    // IPluginFactory2
    pub get_class_info2: unsafe extern "system" fn(
        this: *mut IPluginFactory2,
        index: i32,
        info: *mut PClassInfo2,
    ) -> TResult,
}

/// IPluginFactory3 interface - extends IPluginFactory2 with unicode class info
#[repr(C)]
pub struct IPluginFactory3 {
    pub vtable: *const IPluginFactory3Vtable,
}

/// IPluginFactory3 vtable
#[repr(C)]
#[derive(Clone, Copy)]
pub struct IPluginFactory3Vtable {
    // IUnknown
    pub unknown: IUnknownVtable,
    // IPluginFactory
    pub get_factory_info: unsafe extern "system" fn(
        this: *mut IPluginFactory3,
        info: *mut PFactoryInfo,
    ) -> TResult,
    pub count_classes: unsafe extern "system" fn(
        this: *mut IPluginFactory3,
    ) -> i32,
    pub get_class_info: unsafe extern "system" fn(
        this: *mut IPluginFactory3,
        index: i32,
        info: *mut PClassInfo,
    ) -> TResult,
    pub create_instance: unsafe extern "system" fn(
        this: *mut IPluginFactory3,
        cid: *const TUID,
        iid: *const TUID,
        obj: *mut *mut c_void,
    ) -> TResult,
    // IPluginFactory2
    pub get_class_info2: unsafe extern "system" fn(
        this: *mut IPluginFactory3,
        index: i32,
        info: *mut PClassInfo2,
    ) -> TResult,
    // IPluginFactory3
    pub get_class_info_unicode: unsafe extern "system" fn(
        this: *mut IPluginFactory3,
        index: i32,
        info: *mut PClassInfoW,
    ) -> TResult,
    pub set_host_context: unsafe extern "system" fn(
        this: *mut IPluginFactory3,
        context: *mut c_void,
    ) -> TResult,
}

/// Class entry in the factory
pub struct ClassEntry {
    /// Class ID
    pub cid: TUID,
    /// Category string (e.g., "Audio Module Class")
    pub category: &'static str,
    /// Plugin name
    pub name: String,
    /// Subcategories (e.g., "Fx|Reverb")
    pub subcategories: String,
    /// Vendor name
    pub vendor: String,
    /// Version string
    pub version: String,
    /// Factory function to create plugin instance
    pub create: Box<dyn Fn() -> Box<dyn Plugin> + Send + Sync>,
    /// Controller class ID (for processor class)
    pub controller_cid: Option<TUID>,
}

/// VST3 Plugin Factory implementation
#[repr(C)]
pub struct Vst3PluginFactory {
    /// COM object base
    pub com: ComObject,
    /// Factory info
    pub factory_info: FactoryInfo,
    /// Registered classes
    pub classes: Vec<ClassEntry>,
    /// Host context
    host_context: *mut c_void,
}

/// Factory information
#[derive(Clone)]
pub struct FactoryInfo {
    pub vendor: String,
    pub url: String,
    pub email: String,
}

impl Default for FactoryInfo {
    fn default() -> Self {
        Self {
            vendor: "Pulse".to_string(),
            url: "https://github.com/pulse".to_string(),
            email: "info@pulse.dev".to_string(),
        }
    }
}

impl Vst3PluginFactory {
    /// Create a new plugin factory
    pub fn new(info: FactoryInfo) -> Box<Self> {
        Box::new(Self {
            com: ComObject::new(&FACTORY_VTABLE as *const _ as *const IUnknownVtable),
            factory_info: info,
            classes: Vec::new(),
            host_context: std::ptr::null_mut(),
        })
    }

    /// Register a processor class
    pub fn register_processor<F>(
        &mut self,
        cid: TUID,
        controller_cid: TUID,
        info: &PluginInfo,
        create: F,
    ) where
        F: Fn() -> Box<dyn Plugin> + Send + Sync + 'static,
    {
        let subcategories = category_to_vst3(info.category).to_string();
        self.classes.push(ClassEntry {
            cid,
            category: "Audio Module Class",
            name: info.name.clone(),
            subcategories,
            vendor: info.vendor.clone(),
            version: info.version.clone(),
            create: Box::new(create),
            controller_cid: Some(controller_cid),
        });
    }

    /// Register a controller class
    pub fn register_controller<F>(
        &mut self,
        cid: TUID,
        info: &PluginInfo,
        create: F,
    ) where
        F: Fn() -> Box<dyn Plugin> + Send + Sync + 'static,
    {
        self.classes.push(ClassEntry {
            cid,
            category: "Component Controller Class",
            name: format!("{} Controller", info.name),
            subcategories: String::new(),
            vendor: info.vendor.clone(),
            version: info.version.clone(),
            create: Box::new(create),
            controller_cid: None,
        });
    }

    /// Get class count
    pub fn class_count(&self) -> i32 {
        self.classes.len() as i32
    }

    /// Get class by index
    pub fn get_class(&self, index: usize) -> Option<&ClassEntry> {
        self.classes.get(index)
    }

    /// Find class by CID
    pub fn find_class(&self, cid: &TUID) -> Option<&ClassEntry> {
        self.classes.iter().find(|c| tuid_eq(&c.cid, cid))
    }
}

// Factory vtable that implements IPluginFactory3
static FACTORY_VTABLE: IPluginFactory3Vtable = IPluginFactory3Vtable {
    unknown: IUnknownVtable {
        query_interface: factory_query_interface,
        add_ref: factory_add_ref,
        release: factory_release,
    },
    get_factory_info: factory_get_factory_info,
    count_classes: factory_count_classes,
    get_class_info: factory_get_class_info,
    create_instance: factory_create_instance,
    get_class_info2: factory_get_class_info2,
    get_class_info_unicode: factory_get_class_info_unicode,
    set_host_context: factory_set_host_context,
};

unsafe extern "system" fn factory_query_interface(
    this: *mut c_void,
    iid: *const TUID,
    obj: *mut *mut c_void,
) -> TResult {
    if this.is_null() || iid.is_null() || obj.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let requested_iid = &*iid;

    if tuid_eq(requested_iid, &iid::FUNKNOWN)
        || tuid_eq(requested_iid, &iid::IPLUGIN_FACTORY)
        || tuid_eq(requested_iid, &iid::IPLUGIN_FACTORY2)
        || tuid_eq(requested_iid, &iid::IPLUGIN_FACTORY3)
    {
        let factory = this as *mut Vst3PluginFactory;
        (*factory).com.add_ref();
        *obj = this;
        return K_RESULT_OK;
    }

    *obj = std::ptr::null_mut();
    K_NOT_IMPLEMENTED
}

unsafe extern "system" fn factory_add_ref(this: *mut c_void) -> u32 {
    if this.is_null() {
        return 0;
    }
    let factory = this as *mut Vst3PluginFactory;
    (*factory).com.add_ref()
}

unsafe extern "system" fn factory_release(this: *mut c_void) -> u32 {
    if this.is_null() {
        return 0;
    }
    let factory = this as *mut Vst3PluginFactory;
    let count = (*factory).com.release();
    if count == 0 {
        drop(Box::from_raw(factory));
    }
    count
}

unsafe extern "system" fn factory_get_factory_info(
    this: *mut IPluginFactory3,
    info: *mut PFactoryInfo,
) -> TResult {
    if this.is_null() || info.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let factory = this as *const Vst3PluginFactory;
    let factory_info = &(*factory).factory_info;

    *info = PFactoryInfo::new(
        &factory_info.vendor,
        &factory_info.url,
        &factory_info.email,
        K_UNICODE,
    );

    K_RESULT_OK
}

unsafe extern "system" fn factory_count_classes(this: *mut IPluginFactory3) -> i32 {
    if this.is_null() {
        return 0;
    }

    let factory = this as *const Vst3PluginFactory;
    (*factory).class_count()
}

unsafe extern "system" fn factory_get_class_info(
    this: *mut IPluginFactory3,
    index: i32,
    info: *mut PClassInfo,
) -> TResult {
    if this.is_null() || info.is_null() || index < 0 {
        return K_INVALID_ARGUMENT;
    }

    let factory = this as *const Vst3PluginFactory;

    if let Some(class) = (*factory).get_class(index as usize) {
        *info = PClassInfo::new(class.cid, class.category, &class.name);
        K_RESULT_OK
    } else {
        K_INVALID_ARGUMENT
    }
}

pub(crate) unsafe extern "system" fn factory_create_instance(
    this: *mut IPluginFactory3,
    cid: *const TUID,
    iid: *const TUID,
    obj: *mut *mut c_void,
) -> TResult {
    if this.is_null() || cid.is_null() || iid.is_null() || obj.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let factory = this as *const Vst3PluginFactory;
    let class_id = &*cid;
    let interface_id = &*iid;

    if let Some(class) = (*factory).find_class(class_id) {
        // Create the plugin instance
        let plugin = (class.create)();

        // Check what interface is requested
        if tuid_eq(interface_id, &iid::ICOMPONENT) || tuid_eq(interface_id, &iid::FUNKNOWN) {
            // Create processor adapter
            let controller_cid = class.controller_cid.unwrap_or([0; 16]);
            let adapter = Vst3Adapter::new(plugin, controller_cid);
            *obj = Box::into_raw(adapter) as *mut c_void;
            return K_RESULT_OK;
        } else if tuid_eq(interface_id, &iid::IEDIT_CONTROLLER) {
            // Create controller
            let params = plugin.parameters();
            let param_state = super::adapter::SharedParameterState::new(&params);
            let param_state = std::sync::Arc::new(std::sync::Mutex::new(param_state));
            let controller = Vst3EditController::new(params, param_state);
            *obj = Box::into_raw(controller) as *mut c_void;
            return K_RESULT_OK;
        }
    }

    *obj = std::ptr::null_mut();
    K_NOT_IMPLEMENTED
}

unsafe extern "system" fn factory_get_class_info2(
    this: *mut IPluginFactory3,
    index: i32,
    info: *mut PClassInfo2,
) -> TResult {
    if this.is_null() || info.is_null() || index < 0 {
        return K_INVALID_ARGUMENT;
    }

    let factory = this as *const Vst3PluginFactory;

    if let Some(class) = (*factory).get_class(index as usize) {
        *info = PClassInfo2::new(
            class.cid,
            class.category,
            &class.name,
            &class.subcategories,
            &class.vendor,
            &class.version,
        );
        K_RESULT_OK
    } else {
        K_INVALID_ARGUMENT
    }
}

unsafe extern "system" fn factory_get_class_info_unicode(
    this: *mut IPluginFactory3,
    index: i32,
    info: *mut PClassInfoW,
) -> TResult {
    if this.is_null() || info.is_null() || index < 0 {
        return K_INVALID_ARGUMENT;
    }

    let factory = this as *const Vst3PluginFactory;

    if let Some(class) = (*factory).get_class(index as usize) {
        let mut result = PClassInfoW::default();
        result.cid = class.cid;
        result.cardinality = K_MANY_INSTANCES;
        copy_str_to_cstr(class.category, &mut result.category);
        copy_str_to_u16(&class.name, &mut result.name);
        copy_str_to_cstr(&class.subcategories, &mut result.sub_categories);
        copy_str_to_u16(&class.vendor, &mut result.vendor);
        copy_str_to_u16(&class.version, &mut result.version);
        copy_str_to_u16("VST 3.7", &mut result.sdk_version);

        *info = result;
        K_RESULT_OK
    } else {
        K_INVALID_ARGUMENT
    }
}

unsafe extern "system" fn factory_set_host_context(
    this: *mut IPluginFactory3,
    context: *mut c_void,
) -> TResult {
    if this.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let factory = this as *mut Vst3PluginFactory;
    (*factory).host_context = context;

    K_RESULT_OK
}

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

/// Generate a TUID from a plugin ID string
pub fn generate_tuid(plugin_id: &str, suffix: &str) -> TUID {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    plugin_id.hash(&mut hasher);
    suffix.hash(&mut hasher);
    let hash1 = hasher.finish();

    let mut hasher2 = DefaultHasher::new();
    hash1.hash(&mut hasher2);
    "pulse-vst3".hash(&mut hasher2);
    let hash2 = hasher2.finish();

    let mut tuid = [0u8; 16];
    tuid[..8].copy_from_slice(&hash1.to_le_bytes());
    tuid[8..].copy_from_slice(&hash2.to_le_bytes());
    tuid
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::AudioBuffer;
    use crate::plugin::{PluginCategory, PluginConfig};
    use crate::process::ProcessContext;

    struct TestPlugin {
        name: String,
    }

    impl TestPlugin {
        fn new(name: &str) -> Self {
            Self { name: name.to_string() }
        }
    }

    impl Plugin for TestPlugin {
        fn info(&self) -> PluginInfo {
            PluginInfo {
                id: "test.factory.plugin".to_string(),
                name: self.name.clone(),
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

        fn parameters(&self) -> Vec<crate::param::ParamInfo> {
            vec![]
        }

        fn set_parameter(&mut self, _id: u32, _value: f32) {}
        fn get_parameter(&self, _id: u32) -> f32 { 0.0 }
        fn get_state(&self) -> Vec<u8> { vec![] }
        fn set_state(&mut self, _data: &[u8]) -> crate::Result<()> { Ok(()) }
        fn reset(&mut self) {}
    }

    #[test]
    fn test_iplugin_factory_vtable_layout() {
        // Verify vtable is pointer-aligned
        assert_eq!(
            std::mem::align_of::<IPluginFactoryVtable>(),
            std::mem::align_of::<*const c_void>()
        );

        // IPluginFactoryVtable should have:
        // - 3 pointers from IUnknown
        // - 4 pointers from IPluginFactory
        // Total: 7 pointers
        let expected_size = 7 * std::mem::size_of::<*const c_void>();
        assert_eq!(std::mem::size_of::<IPluginFactoryVtable>(), expected_size);
    }

    #[test]
    fn test_iplugin_factory2_vtable_layout() {
        // IPluginFactory2Vtable should have:
        // - 3 pointers from IUnknown
        // - 4 pointers from IPluginFactory
        // - 1 pointer from IPluginFactory2
        // Total: 8 pointers
        let expected_size = 8 * std::mem::size_of::<*const c_void>();
        assert_eq!(std::mem::size_of::<IPluginFactory2Vtable>(), expected_size);
    }

    #[test]
    fn test_iplugin_factory3_vtable_layout() {
        // IPluginFactory3Vtable should have:
        // - 3 pointers from IUnknown
        // - 4 pointers from IPluginFactory
        // - 1 pointer from IPluginFactory2
        // - 2 pointers from IPluginFactory3
        // Total: 10 pointers
        let expected_size = 10 * std::mem::size_of::<*const c_void>();
        assert_eq!(std::mem::size_of::<IPluginFactory3Vtable>(), expected_size);
    }

    #[test]
    fn test_factory_inherits_iunknown() {
        // All factory interfaces should have IUnknown at offset 0
        assert_eq!(std::mem::offset_of!(IPluginFactoryVtable, unknown), 0);
        assert_eq!(std::mem::offset_of!(IPluginFactory2Vtable, unknown), 0);
        assert_eq!(std::mem::offset_of!(IPluginFactory3Vtable, unknown), 0);
    }

    #[test]
    fn test_factory_method_order() {
        let ptr_size = std::mem::size_of::<*const c_void>();

        // IPluginFactory methods start after IUnknown
        let get_factory_info_offset = std::mem::offset_of!(IPluginFactoryVtable, get_factory_info);
        assert_eq!(get_factory_info_offset, 3 * ptr_size);

        let count_classes_offset = std::mem::offset_of!(IPluginFactoryVtable, count_classes);
        assert_eq!(count_classes_offset, 4 * ptr_size);

        let get_class_info_offset = std::mem::offset_of!(IPluginFactoryVtable, get_class_info);
        assert_eq!(get_class_info_offset, 5 * ptr_size);

        let create_instance_offset = std::mem::offset_of!(IPluginFactoryVtable, create_instance);
        assert_eq!(create_instance_offset, 6 * ptr_size);
    }

    #[test]
    fn test_factory2_extends_factory() {
        let ptr_size = std::mem::size_of::<*const c_void>();

        // get_class_info2 should come after IPluginFactory methods
        let get_class_info2_offset = std::mem::offset_of!(IPluginFactory2Vtable, get_class_info2);
        assert_eq!(get_class_info2_offset, 7 * ptr_size);
    }

    #[test]
    fn test_factory3_extends_factory2() {
        let ptr_size = std::mem::size_of::<*const c_void>();

        // IPluginFactory3 methods come after IPluginFactory2
        let get_class_info_unicode_offset = std::mem::offset_of!(IPluginFactory3Vtable, get_class_info_unicode);
        assert_eq!(get_class_info_unicode_offset, 8 * ptr_size);

        let set_host_context_offset = std::mem::offset_of!(IPluginFactory3Vtable, set_host_context);
        assert_eq!(set_host_context_offset, 9 * ptr_size);
    }

    #[test]
    fn test_factory_creation() {
        let info = FactoryInfo {
            vendor: "Test Vendor".to_string(),
            url: "https://test.com".to_string(),
            email: "test@test.com".to_string(),
        };

        let factory = Vst3PluginFactory::new(info);
        assert_eq!(factory.class_count(), 0);
        assert_eq!(factory.factory_info.vendor, "Test Vendor");
    }

    #[test]
    fn test_factory_register_processor() {
        let info = FactoryInfo::default();
        let mut factory = Vst3PluginFactory::new(info);

        let plugin_info = PluginInfo {
            id: "test.plugin".to_string(),
            name: "Test Plugin".to_string(),
            vendor: "Test".to_string(),
            version: "1.0.0".to_string(),
            category: PluginCategory::Effect,
            inputs: 2,
            outputs: 2,
        };

        let processor_cid: TUID = [1; 16];
        let controller_cid: TUID = [2; 16];

        factory.register_processor(processor_cid, controller_cid, &plugin_info, || {
            Box::new(TestPlugin::new("Test"))
        });

        assert_eq!(factory.class_count(), 1);

        let class = factory.get_class(0).unwrap();
        assert_eq!(class.cid, processor_cid);
        assert_eq!(class.name, "Test Plugin");
        assert_eq!(class.category, "Audio Module Class");
        assert_eq!(class.controller_cid, Some(controller_cid));
    }

    #[test]
    fn test_factory_register_controller() {
        let info = FactoryInfo::default();
        let mut factory = Vst3PluginFactory::new(info);

        let plugin_info = PluginInfo {
            id: "test.plugin".to_string(),
            name: "Test Plugin".to_string(),
            vendor: "Test".to_string(),
            version: "1.0.0".to_string(),
            category: PluginCategory::Effect,
            inputs: 2,
            outputs: 2,
        };

        let controller_cid: TUID = [3; 16];

        factory.register_controller(controller_cid, &plugin_info, || {
            Box::new(TestPlugin::new("Test"))
        });

        assert_eq!(factory.class_count(), 1);

        let class = factory.get_class(0).unwrap();
        assert_eq!(class.cid, controller_cid);
        assert_eq!(class.name, "Test Plugin Controller");
        assert_eq!(class.category, "Component Controller Class");
        assert_eq!(class.controller_cid, None);
    }

    #[test]
    fn test_factory_find_class() {
        let info = FactoryInfo::default();
        let mut factory = Vst3PluginFactory::new(info);

        let plugin_info = PluginInfo {
            id: "test.plugin".to_string(),
            name: "Test Plugin".to_string(),
            vendor: "Test".to_string(),
            version: "1.0.0".to_string(),
            category: PluginCategory::Effect,
            inputs: 2,
            outputs: 2,
        };

        let cid: TUID = [4; 16];
        factory.register_processor(cid, [5; 16], &plugin_info, || {
            Box::new(TestPlugin::new("Test"))
        });

        let found = factory.find_class(&cid);
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "Test Plugin");

        let not_found = factory.find_class(&[99; 16]);
        assert!(not_found.is_none());
    }

    #[test]
    fn test_factory_get_factory_info() {
        let info = FactoryInfo {
            vendor: "My Vendor".to_string(),
            url: "https://example.com".to_string(),
            email: "info@example.com".to_string(),
        };

        let factory = Vst3PluginFactory::new(info);
        let ptr = Box::into_raw(factory) as *mut IPluginFactory3;

        unsafe {
            let mut factory_info = PFactoryInfo::default();
            let result = factory_get_factory_info(ptr, &mut factory_info);
            assert_eq!(result, K_RESULT_OK);

            let vendor: String = factory_info.vendor.iter()
                .take_while(|&&c| c != 0)
                .map(|&c| c as u8 as char)
                .collect();
            assert_eq!(vendor, "My Vendor");

            factory_release(ptr as *mut c_void);
        }
    }

    #[test]
    fn test_factory_count_classes() {
        let info = FactoryInfo::default();
        let mut factory = Vst3PluginFactory::new(info);

        let plugin_info = PluginInfo {
            id: "test".to_string(),
            name: "Test".to_string(),
            vendor: "Test".to_string(),
            version: "1.0.0".to_string(),
            category: PluginCategory::Effect,
            inputs: 2,
            outputs: 2,
        };

        factory.register_processor([1; 16], [2; 16], &plugin_info, || {
            Box::new(TestPlugin::new("Test"))
        });
        factory.register_controller([2; 16], &plugin_info, || {
            Box::new(TestPlugin::new("Test"))
        });

        let ptr = Box::into_raw(factory) as *mut IPluginFactory3;

        unsafe {
            let count = factory_count_classes(ptr);
            assert_eq!(count, 2);

            factory_release(ptr as *mut c_void);
        }
    }

    #[test]
    fn test_factory_get_class_info() {
        let info = FactoryInfo::default();
        let mut factory = Vst3PluginFactory::new(info);

        let plugin_info = PluginInfo {
            id: "test".to_string(),
            name: "Test Plugin".to_string(),
            vendor: "Test".to_string(),
            version: "1.0.0".to_string(),
            category: PluginCategory::Effect,
            inputs: 2,
            outputs: 2,
        };

        let cid: TUID = [6; 16];
        factory.register_processor(cid, [7; 16], &plugin_info, || {
            Box::new(TestPlugin::new("Test"))
        });

        let ptr = Box::into_raw(factory) as *mut IPluginFactory3;

        unsafe {
            let mut class_info = PClassInfo::default();
            let result = factory_get_class_info(ptr, 0, &mut class_info);
            assert_eq!(result, K_RESULT_OK);
            assert_eq!(class_info.cid, cid);

            // Invalid index
            let result = factory_get_class_info(ptr, 10, &mut class_info);
            assert_eq!(result, K_INVALID_ARGUMENT);

            factory_release(ptr as *mut c_void);
        }
    }

    #[test]
    fn test_factory_create_instance() {
        let info = FactoryInfo::default();
        let mut factory = Vst3PluginFactory::new(info);

        let plugin_info = PluginInfo {
            id: "test".to_string(),
            name: "Test Plugin".to_string(),
            vendor: "Test".to_string(),
            version: "1.0.0".to_string(),
            category: PluginCategory::Effect,
            inputs: 2,
            outputs: 2,
        };

        let processor_cid: TUID = [8; 16];
        let controller_cid: TUID = [9; 16];

        factory.register_processor(processor_cid, controller_cid, &plugin_info, || {
            Box::new(TestPlugin::new("Test"))
        });

        let ptr = Box::into_raw(factory) as *mut IPluginFactory3;

        unsafe {
            let mut obj: *mut c_void = std::ptr::null_mut();

            // Create IComponent instance
            let result = factory_create_instance(
                ptr,
                &processor_cid,
                &iid::ICOMPONENT,
                &mut obj,
            );
            assert_eq!(result, K_RESULT_OK);
            assert!(!obj.is_null());

            // Release the created instance
            let adapter = obj as *mut Vst3Adapter;
            (*adapter).com.release();
            drop(Box::from_raw(adapter));

            factory_release(ptr as *mut c_void);
        }
    }

    #[test]
    fn test_factory_query_interface() {
        let info = FactoryInfo::default();
        let factory = Vst3PluginFactory::new(info);
        let ptr = Box::into_raw(factory) as *mut c_void;

        unsafe {
            let mut result: *mut c_void = std::ptr::null_mut();

            // Query IPluginFactory
            let status = factory_query_interface(ptr, &iid::IPLUGIN_FACTORY, &mut result);
            assert_eq!(status, K_RESULT_OK);
            factory_release(result);

            // Query IPluginFactory2
            let status = factory_query_interface(ptr, &iid::IPLUGIN_FACTORY2, &mut result);
            assert_eq!(status, K_RESULT_OK);
            factory_release(result);

            // Query IPluginFactory3
            let status = factory_query_interface(ptr, &iid::IPLUGIN_FACTORY3, &mut result);
            assert_eq!(status, K_RESULT_OK);
            factory_release(result);

            // Query FUnknown
            let status = factory_query_interface(ptr, &iid::FUNKNOWN, &mut result);
            assert_eq!(status, K_RESULT_OK);
            factory_release(result);

            // Query unsupported
            let unknown_iid: TUID = [0xFF; 16];
            let status = factory_query_interface(ptr, &unknown_iid, &mut result);
            assert_eq!(status, K_NOT_IMPLEMENTED);

            factory_release(ptr);
        }
    }

    #[test]
    fn test_factory_get_class_info2() {
        let info = FactoryInfo::default();
        let mut factory = Vst3PluginFactory::new(info);

        let plugin_info = PluginInfo {
            id: "test".to_string(),
            name: "Test Plugin".to_string(),
            vendor: "My Vendor".to_string(),
            version: "2.0.0".to_string(),
            category: PluginCategory::Effect,
            inputs: 2,
            outputs: 2,
        };

        factory.register_processor([10; 16], [11; 16], &plugin_info, || {
            Box::new(TestPlugin::new("Test"))
        });

        let ptr = Box::into_raw(factory) as *mut IPluginFactory3;

        unsafe {
            let mut class_info = PClassInfo2::default();
            let result = factory_get_class_info2(ptr, 0, &mut class_info);
            assert_eq!(result, K_RESULT_OK);

            let vendor: String = class_info.vendor.iter()
                .take_while(|&&c| c != 0)
                .map(|&c| c as u8 as char)
                .collect();
            assert_eq!(vendor, "My Vendor");

            factory_release(ptr as *mut c_void);
        }
    }

    #[test]
    fn test_generate_tuid() {
        let tuid1 = generate_tuid("com.example.plugin", "processor");
        let tuid2 = generate_tuid("com.example.plugin", "processor");
        let tuid3 = generate_tuid("com.example.plugin", "controller");

        // Same inputs should produce same output
        assert_eq!(tuid1, tuid2);

        // Different suffix should produce different output
        assert_ne!(tuid1, tuid3);

        // TUIDs should be 16 bytes
        assert_eq!(tuid1.len(), 16);
    }

    #[test]
    fn test_factory_set_host_context() {
        let info = FactoryInfo::default();
        let factory = Vst3PluginFactory::new(info);
        let ptr = Box::into_raw(factory) as *mut IPluginFactory3;

        unsafe {
            let fake_context = 0x12345678 as *mut c_void;
            let result = factory_set_host_context(ptr, fake_context);
            assert_eq!(result, K_RESULT_OK);

            let factory_ptr = ptr as *const Vst3PluginFactory;
            assert_eq!((*factory_ptr).host_context, fake_context);

            factory_release(ptr as *mut c_void);
        }
    }
}
