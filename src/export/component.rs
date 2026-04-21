//! VST3 IComponent interface definition

use std::ffi::c_void;

use super::com::IUnknownVtable;
use super::types::{TResult, BusInfo, RoutingInfo};

/// IBStream interface for state handling
#[repr(C)]
pub struct IBStream {
    pub vtable: *const IBStreamVtable,
}

/// IBStream vtable
#[repr(C)]
#[derive(Clone, Copy)]
pub struct IBStreamVtable {
    pub unknown: IUnknownVtable,
    pub read: unsafe extern "system" fn(
        this: *mut IBStream,
        buffer: *mut c_void,
        num_bytes: i32,
        num_bytes_read: *mut i32,
    ) -> TResult,
    pub write: unsafe extern "system" fn(
        this: *mut IBStream,
        buffer: *const c_void,
        num_bytes: i32,
        num_bytes_written: *mut i32,
    ) -> TResult,
    pub seek: unsafe extern "system" fn(
        this: *mut IBStream,
        pos: i64,
        mode: i32,
        result: *mut i64,
    ) -> TResult,
    pub tell: unsafe extern "system" fn(
        this: *mut IBStream,
        pos: *mut i64,
    ) -> TResult,
}

/// IPluginBase interface - base for IComponent and IEditController
#[repr(C)]
pub struct IPluginBase {
    pub vtable: *const IPluginBaseVtable,
}

/// IPluginBase vtable
#[repr(C)]
#[derive(Clone, Copy)]
pub struct IPluginBaseVtable {
    pub unknown: IUnknownVtable,
    pub initialize: unsafe extern "system" fn(
        this: *mut IPluginBase,
        context: *mut c_void,
    ) -> TResult,
    pub terminate: unsafe extern "system" fn(
        this: *mut IPluginBase,
    ) -> TResult,
}

/// IComponent interface
#[repr(C)]
pub struct IComponent {
    pub vtable: *const IComponentVtable,
}

/// IComponent vtable - extends IPluginBase
#[repr(C)]
#[derive(Clone, Copy)]
pub struct IComponentVtable {
    // IUnknown
    pub unknown: IUnknownVtable,
    // IPluginBase
    pub initialize: unsafe extern "system" fn(
        this: *mut IComponent,
        context: *mut c_void,
    ) -> TResult,
    pub terminate: unsafe extern "system" fn(
        this: *mut IComponent,
    ) -> TResult,
    // IComponent
    pub get_controller_class_id: unsafe extern "system" fn(
        this: *mut IComponent,
        class_id: *mut [u8; 16],
    ) -> TResult,
    pub set_io_mode: unsafe extern "system" fn(
        this: *mut IComponent,
        mode: i32,
    ) -> TResult,
    pub get_bus_count: unsafe extern "system" fn(
        this: *mut IComponent,
        media_type: i32,
        dir: i32,
    ) -> i32,
    pub get_bus_info: unsafe extern "system" fn(
        this: *mut IComponent,
        media_type: i32,
        dir: i32,
        index: i32,
        info: *mut BusInfo,
    ) -> TResult,
    pub get_routing_info: unsafe extern "system" fn(
        this: *mut IComponent,
        in_info: *mut RoutingInfo,
        out_info: *mut RoutingInfo,
    ) -> TResult,
    pub activate_bus: unsafe extern "system" fn(
        this: *mut IComponent,
        media_type: i32,
        dir: i32,
        index: i32,
        state: u8,
    ) -> TResult,
    pub set_active: unsafe extern "system" fn(
        this: *mut IComponent,
        state: u8,
    ) -> TResult,
    pub set_state: unsafe extern "system" fn(
        this: *mut IComponent,
        state: *mut IBStream,
    ) -> TResult,
    pub get_state: unsafe extern "system" fn(
        this: *mut IComponent,
        state: *mut IBStream,
    ) -> TResult,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icomponent_vtable_layout() {
        // Verify vtable is pointer-aligned
        assert_eq!(
            std::mem::align_of::<IComponentVtable>(),
            std::mem::align_of::<*const c_void>()
        );

        // IComponentVtable should have:
        // - 3 pointers from IUnknown (query_interface, add_ref, release)
        // - 2 pointers from IPluginBase (initialize, terminate)
        // - 9 pointers from IComponent
        // Total: 14 pointers
        let expected_size = 14 * std::mem::size_of::<*const c_void>();
        assert_eq!(std::mem::size_of::<IComponentVtable>(), expected_size);
    }

    #[test]
    fn test_iplugin_base_vtable_layout() {
        // IPluginBase has IUnknown (3) + initialize + terminate = 5 pointers
        let expected_size = 5 * std::mem::size_of::<*const c_void>();
        assert_eq!(std::mem::size_of::<IPluginBaseVtable>(), expected_size);
    }

    #[test]
    fn test_ibstream_vtable_layout() {
        // IBStream has IUnknown (3) + read + write + seek + tell = 7 pointers
        let expected_size = 7 * std::mem::size_of::<*const c_void>();
        assert_eq!(std::mem::size_of::<IBStreamVtable>(), expected_size);
    }

    #[test]
    fn test_icomponent_inherits_iunknown() {
        // Verify that the first field is IUnknownVtable for correct COM inheritance
        let vtable_offset = std::mem::offset_of!(IComponentVtable, unknown);
        assert_eq!(vtable_offset, 0);
    }
}
