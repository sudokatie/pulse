//! VST3 adapter - wraps a Pulse Plugin for VST3 export

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use crate::buffer::AudioBuffer;
use crate::param::ParamInfo;
use crate::plugin::{Plugin, PluginConfig, PluginInfo};
use crate::process::ProcessContext;

use super::com::{ComObject, IUnknownVtable};
use super::component::{IComponent, IComponentVtable, IBStream};
use super::param_map::Vst3ParameterMapping;
use super::state_bridge::read_state_from_stream;
use super::types::{
    iid, tuid_eq, BusInfo, RoutingInfo, TResult, TUID,
    K_AUDIO, K_INPUT, K_MAIN, K_OUTPUT, K_RESULT_OK, K_INVALID_ARGUMENT,
    K_NOT_IMPLEMENTED, K_NOT_INITIALIZED,
};

/// Shared parameter state between processor and controller
pub struct SharedParameterState {
    /// Current parameter values (normalized 0-1)
    pub values: Vec<f64>,
    /// Parameter mapping
    pub mapping: Vst3ParameterMapping,
}

impl SharedParameterState {
    pub fn new(params: &[ParamInfo]) -> Self {
        let mapping = Vst3ParameterMapping::from_params(params);
        let values: Vec<f64> = params.iter()
            .map(|p| mapping.get_default_normalized(p.id))
            .collect();
        Self { values, mapping }
    }

    pub fn set_normalized(&mut self, param_id: u32, value: f64) {
        if let Some(index) = self.mapping.param_id_to_vst3_index(param_id) {
            if (index as usize) < self.values.len() {
                self.values[index as usize] = value.clamp(0.0, 1.0);
            }
        }
    }

    pub fn get_normalized(&self, param_id: u32) -> f64 {
        if let Some(index) = self.mapping.param_id_to_vst3_index(param_id) {
            if (index as usize) < self.values.len() {
                return self.values[index as usize];
            }
        }
        0.0
    }
}

/// VST3 adapter wrapping a Pulse Plugin
#[repr(C)]
pub struct Vst3Adapter {
    /// COM object base with vtable and ref count
    pub com: ComObject,
    /// The wrapped plugin
    plugin: Box<dyn Plugin>,
    /// Plugin info cache
    info: PluginInfo,
    /// Shared parameter state
    pub param_state: Arc<Mutex<SharedParameterState>>,
    /// Controller class ID
    controller_cid: TUID,
    /// Whether plugin is initialized
    initialized: AtomicBool,
    /// Whether plugin is active
    active: AtomicBool,
    /// Input bus active state
    input_bus_active: AtomicBool,
    /// Output bus active state
    output_bus_active: AtomicBool,
    /// Current sample rate
    sample_rate: AtomicU32,
}

impl Vst3Adapter {
    /// Create a new VST3 adapter wrapping a plugin
    pub fn new(plugin: Box<dyn Plugin>, controller_cid: TUID) -> Box<Self> {
        let info = plugin.info();
        let params = plugin.parameters();
        let param_state = Arc::new(Mutex::new(SharedParameterState::new(&params)));

        Box::new(Self {
            com: ComObject::new(&VST3_ADAPTER_VTABLE as *const _ as *const IUnknownVtable),
            plugin,
            info,
            param_state,
            controller_cid,
            initialized: AtomicBool::new(false),
            active: AtomicBool::new(false),
            input_bus_active: AtomicBool::new(true),
            output_bus_active: AtomicBool::new(true),
            sample_rate: AtomicU32::new(44100),
        })
    }

    /// Get the shared parameter state
    pub fn shared_state(&self) -> Arc<Mutex<SharedParameterState>> {
        Arc::clone(&self.param_state)
    }

    /// Get mutable reference to the plugin
    pub fn plugin_mut(&mut self) -> &mut dyn Plugin {
        &mut *self.plugin
    }

    /// Get reference to the plugin
    pub fn plugin(&self) -> &dyn Plugin {
        &*self.plugin
    }

    /// Get plugin info
    pub fn info(&self) -> &PluginInfo {
        &self.info
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::SeqCst)
    }

    /// Check if active
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    /// Get sample rate
    pub fn sample_rate(&self) -> f32 {
        f32::from_bits(self.sample_rate.load(Ordering::SeqCst))
    }

    /// Set sample rate
    pub fn set_sample_rate(&self, rate: f32) {
        self.sample_rate.store(rate.to_bits(), Ordering::SeqCst);
    }
}

// IComponent vtable implementation
static VST3_ADAPTER_VTABLE: IComponentVtable = IComponentVtable {
    unknown: IUnknownVtable {
        query_interface: adapter_query_interface,
        add_ref: adapter_add_ref,
        release: adapter_release,
    },
    initialize: adapter_initialize,
    terminate: adapter_terminate,
    get_controller_class_id: adapter_get_controller_class_id,
    set_io_mode: adapter_set_io_mode,
    get_bus_count: adapter_get_bus_count,
    get_bus_info: adapter_get_bus_info,
    get_routing_info: adapter_get_routing_info,
    activate_bus: adapter_activate_bus,
    set_active: adapter_set_active,
    set_state: adapter_set_state,
    get_state: adapter_get_state,
};

unsafe extern "system" fn adapter_query_interface(
    this: *mut c_void,
    iid: *const TUID,
    obj: *mut *mut c_void,
) -> TResult {
    if this.is_null() || iid.is_null() || obj.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let requested_iid = &*iid;

    // Check for supported interfaces
    if tuid_eq(requested_iid, &iid::FUNKNOWN)
        || tuid_eq(requested_iid, &iid::IPLUGIN_BASE)
        || tuid_eq(requested_iid, &iid::ICOMPONENT)
    {
        let adapter = this as *mut Vst3Adapter;
        (*adapter).com.add_ref();
        *obj = this;
        return K_RESULT_OK;
    }

    *obj = std::ptr::null_mut();
    K_NOT_IMPLEMENTED
}

unsafe extern "system" fn adapter_add_ref(this: *mut c_void) -> u32 {
    if this.is_null() {
        return 0;
    }
    let adapter = this as *mut Vst3Adapter;
    (*adapter).com.add_ref()
}

unsafe extern "system" fn adapter_release(this: *mut c_void) -> u32 {
    if this.is_null() {
        return 0;
    }
    let adapter = this as *mut Vst3Adapter;
    let count = (*adapter).com.release();
    if count == 0 {
        drop(Box::from_raw(adapter));
    }
    count
}

unsafe extern "system" fn adapter_initialize(
    this: *mut IComponent,
    _context: *mut c_void,
) -> TResult {
    if this.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let adapter = this as *mut Vst3Adapter;

    if (*adapter).initialized.load(Ordering::SeqCst) {
        return K_RESULT_OK;
    }

    let config = PluginConfig {
        sample_rate: (*adapter).sample_rate(),
        max_block_size: 4096,
        inputs: (*adapter).info.inputs,
        outputs: (*adapter).info.outputs,
    };

    if (*adapter).plugin.init(&config).is_ok() {
        (*adapter).initialized.store(true, Ordering::SeqCst);
        K_RESULT_OK
    } else {
        K_NOT_INITIALIZED
    }
}

unsafe extern "system" fn adapter_terminate(this: *mut IComponent) -> TResult {
    if this.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let adapter = this as *mut Vst3Adapter;
    (*adapter).initialized.store(false, Ordering::SeqCst);
    (*adapter).active.store(false, Ordering::SeqCst);
    K_RESULT_OK
}

unsafe extern "system" fn adapter_get_controller_class_id(
    this: *mut IComponent,
    class_id: *mut [u8; 16],
) -> TResult {
    if this.is_null() || class_id.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let adapter = this as *mut Vst3Adapter;
    (*class_id) = (*adapter).controller_cid;
    K_RESULT_OK
}

unsafe extern "system" fn adapter_set_io_mode(
    _this: *mut IComponent,
    _mode: i32,
) -> TResult {
    // Simple mode not supported, but return OK
    K_RESULT_OK
}

unsafe extern "system" fn adapter_get_bus_count(
    this: *mut IComponent,
    media_type: i32,
    dir: i32,
) -> i32 {
    if this.is_null() {
        return 0;
    }

    let adapter = this as *const Vst3Adapter;

    // Only support audio buses
    if media_type != K_AUDIO {
        return 0;
    }

    match dir {
        K_INPUT => if (*adapter).info.inputs > 0 { 1 } else { 0 },
        K_OUTPUT => if (*adapter).info.outputs > 0 { 1 } else { 0 },
        _ => 0,
    }
}

unsafe extern "system" fn adapter_get_bus_info(
    this: *mut IComponent,
    media_type: i32,
    dir: i32,
    index: i32,
    info: *mut BusInfo,
) -> TResult {
    if this.is_null() || info.is_null() {
        return K_INVALID_ARGUMENT;
    }

    if index != 0 || media_type != K_AUDIO {
        return K_INVALID_ARGUMENT;
    }

    let adapter = this as *const Vst3Adapter;
    let plugin_info = &(*adapter).info;

    match dir {
        K_INPUT => {
            if plugin_info.inputs == 0 {
                return K_INVALID_ARGUMENT;
            }
            *info = BusInfo::audio("Audio Input", K_INPUT, plugin_info.inputs as i32, K_MAIN);
        }
        K_OUTPUT => {
            if plugin_info.outputs == 0 {
                return K_INVALID_ARGUMENT;
            }
            *info = BusInfo::audio("Audio Output", K_OUTPUT, plugin_info.outputs as i32, K_MAIN);
        }
        _ => return K_INVALID_ARGUMENT,
    }

    K_RESULT_OK
}

unsafe extern "system" fn adapter_get_routing_info(
    _this: *mut IComponent,
    _in_info: *mut RoutingInfo,
    _out_info: *mut RoutingInfo,
) -> TResult {
    // Simple 1:1 routing - not required for basic plugins
    K_NOT_IMPLEMENTED
}

unsafe extern "system" fn adapter_activate_bus(
    this: *mut IComponent,
    media_type: i32,
    dir: i32,
    index: i32,
    state: u8,
) -> TResult {
    if this.is_null() || index != 0 || media_type != K_AUDIO {
        return K_INVALID_ARGUMENT;
    }

    let adapter = this as *mut Vst3Adapter;
    let active = state != 0;

    match dir {
        K_INPUT => (*adapter).input_bus_active.store(active, Ordering::SeqCst),
        K_OUTPUT => (*adapter).output_bus_active.store(active, Ordering::SeqCst),
        _ => return K_INVALID_ARGUMENT,
    }

    K_RESULT_OK
}

unsafe extern "system" fn adapter_set_active(
    this: *mut IComponent,
    state: u8,
) -> TResult {
    if this.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let adapter = this as *mut Vst3Adapter;
    let active = state != 0;

    if active && !(*adapter).initialized.load(Ordering::SeqCst) {
        return K_NOT_INITIALIZED;
    }

    if !active {
        // Reset plugin when deactivating
        (*adapter).plugin.reset();
    }

    (*adapter).active.store(active, Ordering::SeqCst);
    K_RESULT_OK
}

unsafe extern "system" fn adapter_set_state(
    this: *mut IComponent,
    state: *mut IBStream,
) -> TResult {
    if this.is_null() || state.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let adapter = this as *mut Vst3Adapter;

    match read_state_from_stream(state) {
        Ok(data) => {
            if (*adapter).plugin.set_state(&data).is_ok() {
                K_RESULT_OK
            } else {
                K_INVALID_ARGUMENT
            }
        }
        Err(_) => K_INVALID_ARGUMENT,
    }
}

unsafe extern "system" fn adapter_get_state(
    this: *mut IComponent,
    state: *mut IBStream,
) -> TResult {
    if this.is_null() || state.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let adapter = this as *const Vst3Adapter;
    let data = (*adapter).plugin.get_state();

    use super::state_bridge::write_state_to_stream;
    match write_state_to_stream(state, &data) {
        Ok(_) => K_RESULT_OK,
        Err(_) => K_INVALID_ARGUMENT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestPlugin {
        state_data: Vec<u8>,
        gain: f32,
        initialized: bool,
    }

    impl TestPlugin {
        fn new() -> Self {
            Self {
                state_data: vec![],
                gain: 1.0,
                initialized: false,
            }
        }
    }

    impl Plugin for TestPlugin {
        fn info(&self) -> PluginInfo {
            PluginInfo {
                id: "test.vst3.adapter".to_string(),
                name: "Test VST3 Adapter".to_string(),
                vendor: "Test".to_string(),
                version: "1.0.0".to_string(),
                category: crate::plugin::PluginCategory::Effect,
                inputs: 2,
                outputs: 2,
            }
        }

        fn init(&mut self, _config: &PluginConfig) -> crate::Result<()> {
            self.initialized = true;
            Ok(())
        }

        fn process(&mut self, buffer: &mut AudioBuffer, _ctx: &ProcessContext) {
            for ch in 0..buffer.channels() {
                if let Some(channel) = buffer.channel_mut(ch) {
                    for sample in channel.iter_mut() {
                        *sample *= self.gain;
                    }
                }
            }
        }

        fn parameters(&self) -> Vec<ParamInfo> {
            vec![ParamInfo::float(0, "Gain", 0.0, 2.0, 1.0)]
        }

        fn set_parameter(&mut self, id: u32, value: f32) {
            if id == 0 {
                self.gain = value;
            }
        }

        fn get_parameter(&self, id: u32) -> f32 {
            if id == 0 { self.gain } else { 0.0 }
        }

        fn get_state(&self) -> Vec<u8> {
            self.state_data.clone()
        }

        fn set_state(&mut self, data: &[u8]) -> crate::Result<()> {
            self.state_data = data.to_vec();
            Ok(())
        }

        fn reset(&mut self) {
            self.gain = 1.0;
        }
    }

    #[test]
    fn test_adapter_creation() {
        let plugin = Box::new(TestPlugin::new());
        let cid: TUID = [1; 16];
        let adapter = Vst3Adapter::new(plugin, cid);

        assert_eq!(adapter.info().name, "Test VST3 Adapter");
        assert!(!adapter.is_initialized());
        assert!(!adapter.is_active());
    }

    #[test]
    fn test_adapter_shared_state() {
        let plugin = Box::new(TestPlugin::new());
        let cid: TUID = [1; 16];
        let adapter = Vst3Adapter::new(plugin, cid);

        let state = adapter.shared_state();
        let locked = state.lock().unwrap();
        assert_eq!(locked.mapping.count(), 1);
    }

    #[test]
    fn test_shared_parameter_state() {
        let params = vec![
            ParamInfo::float(0, "Volume", 0.0, 1.0, 0.5),
            ParamInfo::float(1, "Pan", -1.0, 1.0, 0.0),
        ];

        let mut state = SharedParameterState::new(&params);

        // Check defaults
        assert!((state.get_normalized(0) - 0.5).abs() < 0.001);
        assert!((state.get_normalized(1) - 0.5).abs() < 0.001);

        // Set and get
        state.set_normalized(0, 0.75);
        assert!((state.get_normalized(0) - 0.75).abs() < 0.001);

        // Clamping
        state.set_normalized(1, 1.5);
        assert!((state.get_normalized(1) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_adapter_vtable_size() {
        // Verify vtable has correct size for IComponent
        let expected_size = 14 * std::mem::size_of::<*const c_void>();
        assert_eq!(std::mem::size_of::<IComponentVtable>(), expected_size);
    }

    #[test]
    fn test_adapter_bus_count() {
        let plugin = Box::new(TestPlugin::new());
        let cid: TUID = [1; 16];
        let adapter = Vst3Adapter::new(plugin, cid);
        let ptr = Box::into_raw(adapter) as *mut IComponent;

        unsafe {
            // Audio input bus
            let count = adapter_get_bus_count(ptr, K_AUDIO, K_INPUT);
            assert_eq!(count, 1);

            // Audio output bus
            let count = adapter_get_bus_count(ptr, K_AUDIO, K_OUTPUT);
            assert_eq!(count, 1);

            // Event bus (not supported)
            let count = adapter_get_bus_count(ptr, 1, K_INPUT);
            assert_eq!(count, 0);

            drop(Box::from_raw(ptr as *mut Vst3Adapter));
        }
    }

    #[test]
    fn test_adapter_initialize() {
        let plugin = Box::new(TestPlugin::new());
        let cid: TUID = [1; 16];
        let adapter = Vst3Adapter::new(plugin, cid);
        let ptr = Box::into_raw(adapter) as *mut IComponent;

        unsafe {
            assert!(!(*( ptr as *const Vst3Adapter)).is_initialized());

            let result = adapter_initialize(ptr, std::ptr::null_mut());
            assert_eq!(result, K_RESULT_OK);
            assert!((*(ptr as *const Vst3Adapter)).is_initialized());

            // Double init should succeed
            let result = adapter_initialize(ptr, std::ptr::null_mut());
            assert_eq!(result, K_RESULT_OK);

            drop(Box::from_raw(ptr as *mut Vst3Adapter));
        }
    }

    #[test]
    fn test_adapter_set_active() {
        let plugin = Box::new(TestPlugin::new());
        let cid: TUID = [1; 16];
        let adapter = Vst3Adapter::new(plugin, cid);
        let ptr = Box::into_raw(adapter) as *mut IComponent;

        unsafe {
            // Must initialize first
            adapter_initialize(ptr, std::ptr::null_mut());

            let result = adapter_set_active(ptr, 1);
            assert_eq!(result, K_RESULT_OK);
            assert!((*(ptr as *const Vst3Adapter)).is_active());

            let result = adapter_set_active(ptr, 0);
            assert_eq!(result, K_RESULT_OK);
            assert!(!(*(ptr as *const Vst3Adapter)).is_active());

            drop(Box::from_raw(ptr as *mut Vst3Adapter));
        }
    }

    #[test]
    fn test_adapter_set_active_requires_init() {
        let plugin = Box::new(TestPlugin::new());
        let cid: TUID = [1; 16];
        let adapter = Vst3Adapter::new(plugin, cid);
        let ptr = Box::into_raw(adapter) as *mut IComponent;

        unsafe {
            // Should fail without initialization
            let result = adapter_set_active(ptr, 1);
            assert_eq!(result, K_NOT_INITIALIZED);

            drop(Box::from_raw(ptr as *mut Vst3Adapter));
        }
    }

    #[test]
    fn test_adapter_get_bus_info() {
        let plugin = Box::new(TestPlugin::new());
        let cid: TUID = [1; 16];
        let adapter = Vst3Adapter::new(plugin, cid);
        let ptr = Box::into_raw(adapter) as *mut IComponent;

        unsafe {
            let mut info = BusInfo::default();

            let result = adapter_get_bus_info(ptr, K_AUDIO, K_INPUT, 0, &mut info);
            assert_eq!(result, K_RESULT_OK);
            assert_eq!(info.channel_count, 2);
            assert_eq!(info.direction, K_INPUT);

            let result = adapter_get_bus_info(ptr, K_AUDIO, K_OUTPUT, 0, &mut info);
            assert_eq!(result, K_RESULT_OK);
            assert_eq!(info.channel_count, 2);
            assert_eq!(info.direction, K_OUTPUT);

            // Invalid index
            let result = adapter_get_bus_info(ptr, K_AUDIO, K_INPUT, 1, &mut info);
            assert_eq!(result, K_INVALID_ARGUMENT);

            drop(Box::from_raw(ptr as *mut Vst3Adapter));
        }
    }

    #[test]
    fn test_adapter_query_interface() {
        let plugin = Box::new(TestPlugin::new());
        let cid: TUID = [1; 16];
        let adapter = Vst3Adapter::new(plugin, cid);
        let ptr = Box::into_raw(adapter);

        unsafe {
            let mut result: *mut c_void = std::ptr::null_mut();

            // Query FUnknown
            let status = adapter_query_interface(
                ptr as *mut c_void,
                &iid::FUNKNOWN,
                &mut result,
            );
            assert_eq!(status, K_RESULT_OK);
            assert!(!result.is_null());
            adapter_release(result);

            // Query IComponent
            let status = adapter_query_interface(
                ptr as *mut c_void,
                &iid::ICOMPONENT,
                &mut result,
            );
            assert_eq!(status, K_RESULT_OK);
            adapter_release(result);

            // Query unsupported interface
            let unknown_iid: TUID = [0xFF; 16];
            let status = adapter_query_interface(
                ptr as *mut c_void,
                &unknown_iid,
                &mut result,
            );
            assert_eq!(status, K_NOT_IMPLEMENTED);

            adapter_release(ptr as *mut c_void);
        }
    }

    #[test]
    fn test_adapter_controller_class_id() {
        let plugin = Box::new(TestPlugin::new());
        let cid: TUID = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let adapter = Vst3Adapter::new(plugin, cid);
        let ptr = Box::into_raw(adapter) as *mut IComponent;

        unsafe {
            let mut result_cid: TUID = [0; 16];
            let status = adapter_get_controller_class_id(ptr, &mut result_cid);
            assert_eq!(status, K_RESULT_OK);
            assert_eq!(result_cid, cid);

            drop(Box::from_raw(ptr as *mut Vst3Adapter));
        }
    }

    #[test]
    fn test_adapter_activate_bus() {
        let plugin = Box::new(TestPlugin::new());
        let cid: TUID = [1; 16];
        let adapter = Vst3Adapter::new(plugin, cid);
        let ptr = Box::into_raw(adapter) as *mut IComponent;

        unsafe {
            // Deactivate input bus
            let result = adapter_activate_bus(ptr, K_AUDIO, K_INPUT, 0, 0);
            assert_eq!(result, K_RESULT_OK);

            // Activate output bus
            let result = adapter_activate_bus(ptr, K_AUDIO, K_OUTPUT, 0, 1);
            assert_eq!(result, K_RESULT_OK);

            // Invalid bus index
            let result = adapter_activate_bus(ptr, K_AUDIO, K_INPUT, 1, 1);
            assert_eq!(result, K_INVALID_ARGUMENT);

            drop(Box::from_raw(ptr as *mut Vst3Adapter));
        }
    }
}
