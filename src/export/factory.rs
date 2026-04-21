//! VST3 IPluginFactory interface definitions

use std::ffi::c_void;

use super::com::IUnknownVtable;
use super::types::{TResult, TUID, PFactoryInfo, PClassInfo, PClassInfo2, PClassInfoW};

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
