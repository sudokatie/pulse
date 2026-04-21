//! VST3 IEditController implementation

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::param::ParamInfo;

use super::adapter::SharedParameterState;
use super::com::{ComObject, IUnknownVtable};
use super::component::IBStream;
use super::edit_controller::{IEditController, IEditControllerVtable};
use super::param_map::Vst3ParameterMapping;
use super::state_bridge::{read_state_from_stream, write_state_to_stream};
use super::types::{
    iid, tuid_eq, ParameterInfo, TResult, TUID,
    K_CAN_AUTOMATE, K_RESULT_OK, K_INVALID_ARGUMENT, K_NOT_IMPLEMENTED,
};
use super::unit_info::{Vst3UnitInfo, FactoryPresets};

/// VST3 Edit Controller implementation
#[repr(C)]
pub struct Vst3EditController {
    pub com: ComObject,
    /// Parameter info list
    params: Vec<ParamInfo>,
    /// Parameter mapping
    mapping: Vst3ParameterMapping,
    /// Shared parameter state with audio processor
    param_state: Arc<Mutex<SharedParameterState>>,
    /// Component handler (host callback)
    component_handler: *mut c_void,
    /// Whether controller is initialized
    initialized: AtomicBool,
    /// Unit info for preset support
    unit_info: Option<Box<Vst3UnitInfo>>,
}

impl Vst3EditController {
    /// Create a new edit controller
    pub fn new(
        params: Vec<ParamInfo>,
        param_state: Arc<Mutex<SharedParameterState>>,
    ) -> Box<Self> {
        let mapping = Vst3ParameterMapping::from_params(&params);

        Box::new(Self {
            com: ComObject::new(&EDIT_CONTROLLER_VTABLE as *const _ as *const IUnknownVtable),
            params,
            mapping,
            param_state,
            component_handler: std::ptr::null_mut(),
            initialized: AtomicBool::new(false),
            unit_info: None,
        })
    }

    /// Create a new edit controller with preset support
    pub fn with_presets(
        params: Vec<ParamInfo>,
        param_state: Arc<Mutex<SharedParameterState>>,
        plugin_id: impl Into<String>,
        factory_presets: FactoryPresets,
    ) -> Box<Self> {
        let mapping = Vst3ParameterMapping::from_params(&params);
        let unit_info = Vst3UnitInfo::new(plugin_id).with_factory_presets(factory_presets);

        Box::new(Self {
            com: ComObject::new(&EDIT_CONTROLLER_VTABLE as *const _ as *const IUnknownVtable),
            params,
            mapping,
            param_state,
            component_handler: std::ptr::null_mut(),
            initialized: AtomicBool::new(false),
            unit_info: Some(Box::new(unit_info)),
        })
    }

    /// Get parameter count
    pub fn parameter_count(&self) -> usize {
        self.params.len()
    }

    /// Get parameter info by index
    pub fn get_param_info(&self, index: usize) -> Option<&ParamInfo> {
        self.params.get(index)
    }

    /// Get unit info reference for preset support
    pub fn unit_info(&self) -> Option<&Vst3UnitInfo> {
        self.unit_info.as_deref()
    }

    /// Get mutable unit info reference
    pub fn unit_info_mut(&mut self) -> Option<&mut Vst3UnitInfo> {
        self.unit_info.as_deref_mut()
    }
}

// IEditController vtable implementation
static EDIT_CONTROLLER_VTABLE: IEditControllerVtable = IEditControllerVtable {
    unknown: IUnknownVtable {
        query_interface: controller_query_interface,
        add_ref: controller_add_ref,
        release: controller_release,
    },
    initialize: controller_initialize,
    terminate: controller_terminate,
    set_component_state: controller_set_component_state,
    set_state: controller_set_state,
    get_state: controller_get_state,
    get_parameter_count: controller_get_parameter_count,
    get_parameter_info: controller_get_parameter_info,
    get_param_string_by_value: controller_get_param_string_by_value,
    get_param_value_by_string: controller_get_param_value_by_string,
    normalized_param_to_plain: controller_normalized_param_to_plain,
    plain_param_to_normalized: controller_plain_param_to_normalized,
    get_param_normalized: controller_get_param_normalized,
    set_param_normalized: controller_set_param_normalized,
    set_component_handler: controller_set_component_handler,
    create_view: controller_create_view,
};

unsafe extern "system" fn controller_query_interface(
    this: *mut c_void,
    iid: *const TUID,
    obj: *mut *mut c_void,
) -> TResult {
    if this.is_null() || iid.is_null() || obj.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let requested_iid = &*iid;

    if tuid_eq(requested_iid, &iid::FUNKNOWN)
        || tuid_eq(requested_iid, &iid::IPLUGIN_BASE)
        || tuid_eq(requested_iid, &iid::IEDIT_CONTROLLER)
    {
        let controller = this as *mut Vst3EditController;
        (*controller).com.add_ref();
        *obj = this;
        return K_RESULT_OK;
    }

    // Check for IUnitInfo - only supported if we have presets
    if tuid_eq(requested_iid, &iid::IUNIT_INFO) {
        let controller = this as *mut Vst3EditController;
        if let Some(ref unit_info) = (*controller).unit_info {
            (*controller).com.add_ref();
            // Return pointer to the unit_info struct
            *obj = unit_info.as_ref() as *const Vst3UnitInfo as *mut c_void;
            return K_RESULT_OK;
        }
    }

    *obj = std::ptr::null_mut();
    K_NOT_IMPLEMENTED
}

unsafe extern "system" fn controller_add_ref(this: *mut c_void) -> u32 {
    if this.is_null() {
        return 0;
    }
    let controller = this as *mut Vst3EditController;
    (*controller).com.add_ref()
}

unsafe extern "system" fn controller_release(this: *mut c_void) -> u32 {
    if this.is_null() {
        return 0;
    }
    let controller = this as *mut Vst3EditController;
    let count = (*controller).com.release();
    if count == 0 {
        drop(Box::from_raw(controller));
    }
    count
}

unsafe extern "system" fn controller_initialize(
    this: *mut IEditController,
    _context: *mut c_void,
) -> TResult {
    if this.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let controller = this as *mut Vst3EditController;
    (*controller).initialized.store(true, Ordering::SeqCst);

    K_RESULT_OK
}

unsafe extern "system" fn controller_terminate(this: *mut IEditController) -> TResult {
    if this.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let controller = this as *mut Vst3EditController;
    (*controller).initialized.store(false, Ordering::SeqCst);
    (*controller).component_handler = std::ptr::null_mut();

    K_RESULT_OK
}

unsafe extern "system" fn controller_set_component_state(
    this: *mut IEditController,
    state: *mut IBStream,
) -> TResult {
    if this.is_null() {
        return K_INVALID_ARGUMENT;
    }

    if state.is_null() {
        return K_RESULT_OK;
    }

    // Read component state and sync parameter values
    if let Ok(data) = read_state_from_stream(state) {
        let controller = this as *mut Vst3EditController;

        // Try to parse state data and update parameters
        // For now, just accept any state
        let _ = data;
        let _ = controller;
    }

    K_RESULT_OK
}

unsafe extern "system" fn controller_set_state(
    this: *mut IEditController,
    state: *mut IBStream,
) -> TResult {
    if this.is_null() || state.is_null() {
        return K_INVALID_ARGUMENT;
    }

    // Controller-specific state (UI preferences, etc.)
    let _ = read_state_from_stream(state);

    K_RESULT_OK
}

unsafe extern "system" fn controller_get_state(
    this: *mut IEditController,
    state: *mut IBStream,
) -> TResult {
    if this.is_null() || state.is_null() {
        return K_INVALID_ARGUMENT;
    }

    // Write empty controller state for now
    let _ = write_state_to_stream(state, &[]);

    K_RESULT_OK
}

unsafe extern "system" fn controller_get_parameter_count(this: *mut IEditController) -> i32 {
    if this.is_null() {
        return 0;
    }

    let controller = this as *const Vst3EditController;
    (*controller).params.len() as i32
}

unsafe extern "system" fn controller_get_parameter_info(
    this: *mut IEditController,
    param_index: i32,
    info: *mut ParameterInfo,
) -> TResult {
    if this.is_null() || info.is_null() || param_index < 0 {
        return K_INVALID_ARGUMENT;
    }

    let controller = this as *const Vst3EditController;
    let index = param_index as usize;

    if index >= (*controller).params.len() {
        return K_INVALID_ARGUMENT;
    }

    let param = &(&(*controller).params)[index];
    let default_normalized = (*controller).mapping.get_default_normalized(param.id);
    let step_count = (*controller).mapping.get_step_count(param.id);

    let mut vst3_info = ParameterInfo::new(
        param.id,
        &param.name,
        default_normalized,
        K_CAN_AUTOMATE,
    ).with_steps(step_count);

    if let Some(unit) = &param.unit {
        vst3_info = vst3_info.with_unit(unit);
    }

    *info = vst3_info;

    K_RESULT_OK
}

unsafe extern "system" fn controller_get_param_string_by_value(
    this: *mut IEditController,
    id: u32,
    value_normalized: f64,
    string: *mut [u16; 128],
) -> TResult {
    if this.is_null() || string.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let controller = this as *const Vst3EditController;
    let plain = (*controller).mapping.normalized_to_plain(id, value_normalized);

    // Format the value as string
    let formatted = format!("{:.2}", plain);
    let chars: Vec<u16> = formatted.encode_utf16().collect();

    let dst = &mut *string;
    let len = chars.len().min(127);
    for (i, &c) in chars[..len].iter().enumerate() {
        dst[i] = c;
    }
    dst[len] = 0;

    K_RESULT_OK
}

unsafe extern "system" fn controller_get_param_value_by_string(
    this: *mut IEditController,
    id: u32,
    string: *const u16,
    value_normalized: *mut f64,
) -> TResult {
    if this.is_null() || string.is_null() || value_normalized.is_null() {
        return K_INVALID_ARGUMENT;
    }

    // Parse null-terminated UTF-16 string
    let mut len = 0;
    while *string.add(len) != 0 && len < 128 {
        len += 1;
    }

    let slice = std::slice::from_raw_parts(string, len);
    let text = String::from_utf16_lossy(slice);

    if let Ok(plain) = text.trim().parse::<f64>() {
        let controller = this as *const Vst3EditController;
        *value_normalized = (*controller).mapping.plain_to_normalized(id, plain);
        K_RESULT_OK
    } else {
        K_INVALID_ARGUMENT
    }
}

unsafe extern "system" fn controller_normalized_param_to_plain(
    this: *mut IEditController,
    id: u32,
    value_normalized: f64,
) -> f64 {
    if this.is_null() {
        return value_normalized;
    }

    let controller = this as *const Vst3EditController;
    (*controller).mapping.normalized_to_plain(id, value_normalized)
}

unsafe extern "system" fn controller_plain_param_to_normalized(
    this: *mut IEditController,
    id: u32,
    plain_value: f64,
) -> f64 {
    if this.is_null() {
        return plain_value;
    }

    let controller = this as *const Vst3EditController;
    (*controller).mapping.plain_to_normalized(id, plain_value)
}

unsafe extern "system" fn controller_get_param_normalized(
    this: *mut IEditController,
    id: u32,
) -> f64 {
    if this.is_null() {
        return 0.0;
    }

    let controller = this as *const Vst3EditController;

    if let Ok(state) = (*controller).param_state.lock() {
        state.get_normalized(id)
    } else {
        0.0
    }
}

unsafe extern "system" fn controller_set_param_normalized(
    this: *mut IEditController,
    id: u32,
    value: f64,
) -> TResult {
    if this.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let controller = this as *mut Vst3EditController;

    if let Ok(mut state) = (*controller).param_state.lock() {
        state.set_normalized(id, value);
        K_RESULT_OK
    } else {
        K_INVALID_ARGUMENT
    }
}

unsafe extern "system" fn controller_set_component_handler(
    this: *mut IEditController,
    handler: *mut c_void,
) -> TResult {
    if this.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let controller = this as *mut Vst3EditController;
    (*controller).component_handler = handler;

    K_RESULT_OK
}

unsafe extern "system" fn controller_create_view(
    _this: *mut IEditController,
    _name: *const i8,
) -> *mut c_void {
    // No GUI support yet
    std::ptr::null_mut()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_controller() -> Box<Vst3EditController> {
        let params = vec![
            ParamInfo::float(0, "Volume", 0.0, 1.0, 0.5),
            ParamInfo::float(1, "Frequency", 20.0, 20000.0, 1000.0),
            ParamInfo::int(2, "Octave", -2, 2, 0),
            ParamInfo::bool(3, "Bypass", false),
        ];
        let param_state = Arc::new(Mutex::new(SharedParameterState::new(&params)));
        Vst3EditController::new(params, param_state)
    }

    #[test]
    fn test_controller_creation() {
        let controller = create_test_controller();
        assert_eq!(controller.parameter_count(), 4);
        assert!(!controller.initialized.load(Ordering::SeqCst));
    }

    #[test]
    fn test_controller_initialize() {
        let controller = create_test_controller();
        let ptr = Box::into_raw(controller);
        let edit_ptr = ptr as *mut IEditController;
        let void_ptr = ptr as *mut c_void;

        unsafe {
            let result = controller_initialize(edit_ptr, std::ptr::null_mut());
            assert_eq!(result, K_RESULT_OK);
            assert!((*ptr).initialized.load(Ordering::SeqCst));

            controller_release(void_ptr);
        }
    }

    #[test]
    fn test_controller_get_parameter_count() {
        let controller = create_test_controller();
        let ptr = Box::into_raw(controller);
        let edit_ptr = ptr as *mut IEditController;
        let void_ptr = ptr as *mut c_void;

        unsafe {
            let count = controller_get_parameter_count(edit_ptr);
            assert_eq!(count, 4);

            controller_release(void_ptr);
        }
    }

    #[test]
    fn test_controller_get_parameter_info() {
        let controller = create_test_controller();
        let ptr = Box::into_raw(controller);
        let edit_ptr = ptr as *mut IEditController;
        let void_ptr = ptr as *mut c_void;

        unsafe {
            let mut info = ParameterInfo::default();

            let result = controller_get_parameter_info(edit_ptr, 0, &mut info);
            assert_eq!(result, K_RESULT_OK);
            assert_eq!(info.id, 0);
            assert!((info.default_normalized_value - 0.5).abs() < 0.001);

            // Invalid index
            let result = controller_get_parameter_info(edit_ptr, 10, &mut info);
            assert_eq!(result, K_INVALID_ARGUMENT);

            controller_release(void_ptr);
        }
    }

    #[test]
    fn test_controller_set_get_param_normalized() {
        let controller = create_test_controller();
        let ptr = Box::into_raw(controller);
        let edit_ptr = ptr as *mut IEditController;
        let void_ptr = ptr as *mut c_void;

        unsafe {
            // Set parameter
            let result = controller_set_param_normalized(edit_ptr, 0, 0.75);
            assert_eq!(result, K_RESULT_OK);

            // Get parameter
            let value = controller_get_param_normalized(edit_ptr, 0);
            assert!((value - 0.75).abs() < 0.001);

            controller_release(void_ptr);
        }
    }

    #[test]
    fn test_controller_param_conversion() {
        let controller = create_test_controller();
        let ptr = Box::into_raw(controller);
        let edit_ptr = ptr as *mut IEditController;
        let void_ptr = ptr as *mut c_void;

        unsafe {
            // Volume: 0-1 range, so normalized = plain
            let plain = controller_normalized_param_to_plain(edit_ptr, 0, 0.5);
            assert!((plain - 0.5).abs() < 0.001);

            let normalized = controller_plain_param_to_normalized(edit_ptr, 0, 0.5);
            assert!((normalized - 0.5).abs() < 0.001);

            // Frequency: 20-20000 range
            // normalized 0 -> plain 20
            let plain = controller_normalized_param_to_plain(edit_ptr, 1, 0.0);
            assert!((plain - 20.0).abs() < 0.001);

            // normalized 1 -> plain 20000
            let plain = controller_normalized_param_to_plain(edit_ptr, 1, 1.0);
            assert!((plain - 20000.0).abs() < 0.001);

            controller_release(void_ptr);
        }
    }

    #[test]
    fn test_controller_param_string_by_value() {
        let controller = create_test_controller();
        let ptr = Box::into_raw(controller);
        let edit_ptr = ptr as *mut IEditController;
        let void_ptr = ptr as *mut c_void;

        unsafe {
            let mut string: [u16; 128] = [0; 128];

            let result = controller_get_param_string_by_value(edit_ptr, 0, 0.5, &mut string);
            assert_eq!(result, K_RESULT_OK);

            // Convert to Rust string and check
            let len = string.iter().position(|&c| c == 0).unwrap_or(128);
            let text = String::from_utf16_lossy(&string[..len]);
            assert_eq!(text, "0.50");

            controller_release(void_ptr);
        }
    }

    #[test]
    fn test_controller_param_value_by_string() {
        let controller = create_test_controller();
        let ptr = Box::into_raw(controller);
        let edit_ptr = ptr as *mut IEditController;
        let void_ptr = ptr as *mut c_void;

        unsafe {
            let input = "0.75";
            let utf16: Vec<u16> = input.encode_utf16().chain(std::iter::once(0)).collect();
            let mut value: f64 = 0.0;

            let result = controller_get_param_value_by_string(
                edit_ptr,
                0,
                utf16.as_ptr(),
                &mut value,
            );
            assert_eq!(result, K_RESULT_OK);
            assert!((value - 0.75).abs() < 0.001);

            controller_release(void_ptr);
        }
    }

    #[test]
    fn test_controller_query_interface() {
        let controller = create_test_controller();
        let ptr = Box::into_raw(controller) as *mut c_void;

        unsafe {
            let mut result: *mut c_void = std::ptr::null_mut();

            // Query IEditController
            let status = controller_query_interface(
                ptr,
                &iid::IEDIT_CONTROLLER,
                &mut result,
            );
            assert_eq!(status, K_RESULT_OK);
            assert!(!result.is_null());
            controller_release(result);

            // Query FUnknown
            let status = controller_query_interface(
                ptr,
                &iid::FUNKNOWN,
                &mut result,
            );
            assert_eq!(status, K_RESULT_OK);
            controller_release(result);

            // Query IPluginBase
            let status = controller_query_interface(
                ptr,
                &iid::IPLUGIN_BASE,
                &mut result,
            );
            assert_eq!(status, K_RESULT_OK);
            controller_release(result);

            // Query unsupported
            let unknown_iid: TUID = [0xFF; 16];
            let status = controller_query_interface(
                ptr,
                &unknown_iid,
                &mut result,
            );
            assert_eq!(status, K_NOT_IMPLEMENTED);

            controller_release(ptr);
        }
    }

    #[test]
    fn test_controller_set_component_handler() {
        let controller = create_test_controller();
        let ptr = Box::into_raw(controller);
        let edit_ptr = ptr as *mut IEditController;
        let void_ptr = ptr as *mut c_void;

        unsafe {
            let fake_handler = 0x12345678 as *mut c_void;

            let result = controller_set_component_handler(edit_ptr, fake_handler);
            assert_eq!(result, K_RESULT_OK);
            assert_eq!((*ptr).component_handler, fake_handler);

            controller_release(void_ptr);
        }
    }

    #[test]
    fn test_controller_create_view() {
        let controller = create_test_controller();
        let ptr = Box::into_raw(controller);
        let edit_ptr = ptr as *mut IEditController;
        let void_ptr = ptr as *mut c_void;

        unsafe {
            let view = controller_create_view(edit_ptr, std::ptr::null());
            assert!(view.is_null()); // No GUI support yet

            controller_release(void_ptr);
        }
    }

    #[test]
    fn test_controller_terminate() {
        let controller = create_test_controller();
        let ptr = Box::into_raw(controller);
        let edit_ptr = ptr as *mut IEditController;
        let void_ptr = ptr as *mut c_void;

        unsafe {
            controller_initialize(edit_ptr, std::ptr::null_mut());
            assert!((*ptr).initialized.load(Ordering::SeqCst));

            let result = controller_terminate(edit_ptr);
            assert_eq!(result, K_RESULT_OK);
            assert!(!(*ptr).initialized.load(Ordering::SeqCst));

            controller_release(void_ptr);
        }
    }

    #[test]
    fn test_controller_vtable_size() {
        // IEditController vtable: 3 (IUnknown) + 2 (IPluginBase) + 13 (IEditController) = 18
        let expected_size = 18 * std::mem::size_of::<*const c_void>();
        assert_eq!(std::mem::size_of::<IEditControllerVtable>(), expected_size);
    }

    #[test]
    fn test_roundtrip_parameter_value() {
        let controller = create_test_controller();
        let ptr = Box::into_raw(controller);
        let edit_ptr = ptr as *mut IEditController;
        let void_ptr = ptr as *mut c_void;

        unsafe {
            // Set value
            controller_set_param_normalized(edit_ptr, 0, 0.33);

            // Get value back
            let value = controller_get_param_normalized(edit_ptr, 0);
            assert!((value - 0.33).abs() < 0.001);

            // Convert to plain and back
            let plain = controller_normalized_param_to_plain(edit_ptr, 0, value);
            let back = controller_plain_param_to_normalized(edit_ptr, 0, plain);
            assert!((back - 0.33).abs() < 0.001);

            controller_release(void_ptr);
        }
    }

    #[test]
    fn test_shared_state_synchronization() {
        let params = vec![ParamInfo::float(0, "Test", 0.0, 1.0, 0.5)];
        let param_state = Arc::new(Mutex::new(SharedParameterState::new(&params)));

        // Clone for controller
        let state_clone = Arc::clone(&param_state);
        let controller = Vst3EditController::new(params, state_clone);
        let ptr = Box::into_raw(controller);
        let edit_ptr = ptr as *mut IEditController;
        let void_ptr = ptr as *mut c_void;

        unsafe {
            // Set via controller
            controller_set_param_normalized(edit_ptr, 0, 0.8);

            // Check shared state
            let state = param_state.lock().unwrap();
            assert!((state.get_normalized(0) - 0.8).abs() < 0.001);
            drop(state);

            // Modify shared state directly
            {
                let mut state = param_state.lock().unwrap();
                state.set_normalized(0, 0.2);
            }

            // Get via controller
            let value = controller_get_param_normalized(edit_ptr, 0);
            assert!((value - 0.2).abs() < 0.001);

            controller_release(void_ptr);
        }
    }

    #[test]
    fn test_controller_with_presets() {
        use crate::preset::{Preset, PresetBank};

        let params = vec![ParamInfo::float(0, "Volume", 0.0, 1.0, 0.5)];
        let param_state = Arc::new(Mutex::new(SharedParameterState::new(&params)));

        // Create factory presets
        let mut bank = PresetBank::new("Factory", "test.plugin");
        let mut preset1 = Preset::new("test.plugin", "Init");
        preset1.set_param("Volume", 0.5);
        bank.add(preset1);
        let mut preset2 = Preset::new("test.plugin", "Loud");
        preset2.set_param("Volume", 1.0);
        bank.add(preset2);

        let factory_presets = FactoryPresets::new().with_bank(bank);

        let controller = Vst3EditController::with_presets(
            params,
            param_state,
            "test.plugin",
            factory_presets,
        );

        // Should have unit info
        assert!(controller.unit_info().is_some());

        let unit_info = controller.unit_info().unwrap();
        assert_eq!(unit_info.unit_count(), 2); // Root + 1 bank
        assert_eq!(unit_info.program_list_count(), 1); // 1 bank

        // Check preset names
        assert_eq!(unit_info.get_program_name(0, 0), Some("Init"));
        assert_eq!(unit_info.get_program_name(0, 1), Some("Loud"));
    }

    #[test]
    fn test_controller_unit_info_query() {
        use crate::preset::{Preset, PresetBank};

        let params = vec![ParamInfo::float(0, "Volume", 0.0, 1.0, 0.5)];
        let param_state = Arc::new(Mutex::new(SharedParameterState::new(&params)));

        let mut bank = PresetBank::new("Factory", "test.plugin");
        bank.add(Preset::new("test.plugin", "Init"));

        let factory_presets = FactoryPresets::new().with_bank(bank);

        let controller = Vst3EditController::with_presets(
            params,
            param_state,
            "test.plugin",
            factory_presets,
        );
        let ptr = Box::into_raw(controller) as *mut c_void;

        unsafe {
            let mut result: *mut c_void = std::ptr::null_mut();

            // Query IUnitInfo - should succeed since we have presets
            let status = controller_query_interface(ptr, &iid::IUNIT_INFO, &mut result);
            assert_eq!(status, K_RESULT_OK);
            assert!(!result.is_null());
            controller_release(result);

            controller_release(ptr);
        }
    }

    #[test]
    fn test_controller_without_presets_no_unit_info() {
        let controller = create_test_controller();

        // Should not have unit info when created without presets
        assert!(controller.unit_info().is_none());

        let ptr = Box::into_raw(controller) as *mut c_void;

        unsafe {
            let mut result: *mut c_void = std::ptr::null_mut();

            // Query IUnitInfo - should fail since we don't have presets
            let status = controller_query_interface(ptr, &iid::IUNIT_INFO, &mut result);
            assert_eq!(status, K_NOT_IMPLEMENTED);
            assert!(result.is_null());

            controller_release(ptr);
        }
    }
}
