//! VST3 IEditController interface definition

use std::ffi::c_void;

use super::com::IUnknownVtable;
use super::component::IBStream;
use super::types::{TResult, ParameterInfo};

/// IEditController interface
#[repr(C)]
pub struct IEditController {
    pub vtable: *const IEditControllerVtable,
}

/// IEditController vtable - extends IPluginBase
#[repr(C)]
#[derive(Clone, Copy)]
pub struct IEditControllerVtable {
    // IUnknown
    pub unknown: IUnknownVtable,
    // IPluginBase
    pub initialize: unsafe extern "system" fn(
        this: *mut IEditController,
        context: *mut c_void,
    ) -> TResult,
    pub terminate: unsafe extern "system" fn(
        this: *mut IEditController,
    ) -> TResult,
    // IEditController
    pub set_component_state: unsafe extern "system" fn(
        this: *mut IEditController,
        state: *mut IBStream,
    ) -> TResult,
    pub set_state: unsafe extern "system" fn(
        this: *mut IEditController,
        state: *mut IBStream,
    ) -> TResult,
    pub get_state: unsafe extern "system" fn(
        this: *mut IEditController,
        state: *mut IBStream,
    ) -> TResult,
    pub get_parameter_count: unsafe extern "system" fn(
        this: *mut IEditController,
    ) -> i32,
    pub get_parameter_info: unsafe extern "system" fn(
        this: *mut IEditController,
        param_index: i32,
        info: *mut ParameterInfo,
    ) -> TResult,
    pub get_param_string_by_value: unsafe extern "system" fn(
        this: *mut IEditController,
        id: u32,
        value_normalized: f64,
        string: *mut [u16; 128],
    ) -> TResult,
    pub get_param_value_by_string: unsafe extern "system" fn(
        this: *mut IEditController,
        id: u32,
        string: *const u16,
        value_normalized: *mut f64,
    ) -> TResult,
    pub normalized_param_to_plain: unsafe extern "system" fn(
        this: *mut IEditController,
        id: u32,
        value_normalized: f64,
    ) -> f64,
    pub plain_param_to_normalized: unsafe extern "system" fn(
        this: *mut IEditController,
        id: u32,
        plain_value: f64,
    ) -> f64,
    pub get_param_normalized: unsafe extern "system" fn(
        this: *mut IEditController,
        id: u32,
    ) -> f64,
    pub set_param_normalized: unsafe extern "system" fn(
        this: *mut IEditController,
        id: u32,
        value: f64,
    ) -> TResult,
    pub set_component_handler: unsafe extern "system" fn(
        this: *mut IEditController,
        handler: *mut c_void,
    ) -> TResult,
    pub create_view: unsafe extern "system" fn(
        this: *mut IEditController,
        name: *const i8,
    ) -> *mut c_void,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iedit_controller_vtable_layout() {
        // Verify vtable is pointer-aligned
        assert_eq!(
            std::mem::align_of::<IEditControllerVtable>(),
            std::mem::align_of::<*const c_void>()
        );

        // IEditControllerVtable should have:
        // - 3 pointers from IUnknown
        // - 2 pointers from IPluginBase (initialize, terminate)
        // - 13 pointers from IEditController
        // Total: 18 pointers
        let expected_size = 18 * std::mem::size_of::<*const c_void>();
        assert_eq!(std::mem::size_of::<IEditControllerVtable>(), expected_size);
    }

    #[test]
    fn test_iedit_controller_inherits_iunknown() {
        // Verify that the first field is IUnknownVtable
        let vtable_offset = std::mem::offset_of!(IEditControllerVtable, unknown);
        assert_eq!(vtable_offset, 0);
    }

    #[test]
    fn test_iedit_controller_method_order() {
        // Verify method offsets follow VST3 specification order
        let ptr_size = std::mem::size_of::<*const c_void>();

        // After IUnknown (3 ptrs)
        let init_offset = std::mem::offset_of!(IEditControllerVtable, initialize);
        assert_eq!(init_offset, 3 * ptr_size);

        // After initialize
        let term_offset = std::mem::offset_of!(IEditControllerVtable, terminate);
        assert_eq!(term_offset, 4 * ptr_size);

        // After IPluginBase (initialize, terminate)
        let set_comp_state_offset = std::mem::offset_of!(IEditControllerVtable, set_component_state);
        assert_eq!(set_comp_state_offset, 5 * ptr_size);
    }
}
