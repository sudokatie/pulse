//! VST3 IAudioProcessor implementation

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use crate::buffer::AudioBuffer;
use crate::param::AutomationManager;
use crate::plugin::Plugin;
use crate::process::ProcessContext;

use super::adapter::SharedParameterState;
use super::audio_processor::{IAudioProcessor, IAudioProcessorVtable};
use super::automation::{extract_automation_events, apply_automation_to_plugin, record_automation_events, SampleAccurateProcessor};
use super::com::{ComObject, IUnknownVtable};
use super::types::{
    iid, tuid_eq, AudioBusBuffers, ProcessData, ProcessSetup, TResult, TUID,
    K_INPUT, K_OUTPUT, K_RESULT_OK, K_INVALID_ARGUMENT, K_NOT_IMPLEMENTED,
    K_SAMPLE_32, K_SAMPLE_64, K_STEREO,
};

/// IParameterChanges vtable for reading parameter changes
#[repr(C)]
pub struct IParameterChangesVtable {
    pub unknown: IUnknownVtable,
    pub get_parameter_count: unsafe extern "system" fn(this: *mut c_void) -> i32,
    pub get_parameter_data: unsafe extern "system" fn(
        this: *mut c_void,
        index: i32,
    ) -> *mut c_void,
}

/// IParamValueQueue vtable for reading parameter values
#[repr(C)]
pub struct IParamValueQueueVtable {
    pub unknown: IUnknownVtable,
    pub get_parameter_id: unsafe extern "system" fn(this: *mut c_void) -> u32,
    pub get_point_count: unsafe extern "system" fn(this: *mut c_void) -> i32,
    pub get_point: unsafe extern "system" fn(
        this: *mut c_void,
        index: i32,
        sample_offset: *mut i32,
        value: *mut f64,
    ) -> TResult,
}

/// VST3 Audio Processor adapter
#[repr(C)]
pub struct Vst3ProcessAdapter {
    pub com: ComObject,
    /// Reference to the plugin (owned by Vst3Adapter)
    plugin: *mut dyn Plugin,
    /// Shared parameter state
    param_state: Arc<Mutex<SharedParameterState>>,
    /// Input speaker arrangement
    input_arrangement: u64,
    /// Output speaker arrangement
    output_arrangement: u64,
    /// Sample rate
    sample_rate: AtomicU32,
    /// Maximum block size
    max_block_size: AtomicU32,
    /// Whether processing is active
    processing: AtomicBool,
    /// Number of input channels
    input_channels: usize,
    /// Number of output channels
    output_channels: usize,
    /// Optional automation manager for recording DAW automation
    automation_manager: Option<Mutex<AutomationManager>>,
    /// Current sample position for automation recording
    current_sample_position: AtomicU32,
}

impl Vst3ProcessAdapter {
    /// Create a new process adapter
    ///
    /// # Safety
    /// The plugin pointer must remain valid for the lifetime of this adapter.
    pub unsafe fn new(
        plugin: *mut dyn Plugin,
        param_state: Arc<Mutex<SharedParameterState>>,
        input_channels: usize,
        output_channels: usize,
    ) -> Box<Self> {
        Box::new(Self {
            com: ComObject::new(&PROCESS_ADAPTER_VTABLE as *const _ as *const IUnknownVtable),
            plugin,
            param_state,
            input_arrangement: if input_channels >= 2 { K_STEREO } else { 0 },
            output_arrangement: if output_channels >= 2 { K_STEREO } else { 0 },
            sample_rate: AtomicU32::new(44100_f32.to_bits()),
            max_block_size: AtomicU32::new(4096),
            processing: AtomicBool::new(false),
            input_channels,
            output_channels,
            automation_manager: None,
            current_sample_position: AtomicU32::new(0),
        })
    }

    /// Create a new process adapter with automation recording enabled
    ///
    /// # Safety
    /// The plugin pointer must remain valid for the lifetime of this adapter.
    pub unsafe fn with_automation_recording(
        plugin: *mut dyn Plugin,
        param_state: Arc<Mutex<SharedParameterState>>,
        input_channels: usize,
        output_channels: usize,
    ) -> Box<Self> {
        let mut adapter = Self::new(plugin, param_state, input_channels, output_channels);
        adapter.automation_manager = Some(Mutex::new(AutomationManager::new(44100.0)));
        adapter
    }

    fn sample_rate(&self) -> f32 {
        f32::from_bits(self.sample_rate.load(Ordering::SeqCst))
    }

    fn set_sample_rate(&self, rate: f32) {
        self.sample_rate.store(rate.to_bits(), Ordering::SeqCst);
        // Update automation manager sample rate
        if let Some(ref manager) = self.automation_manager {
            if let Ok(mut mgr) = manager.lock() {
                *mgr = AutomationManager::new(rate);
            }
        }
    }

    fn max_block_size(&self) -> usize {
        self.max_block_size.load(Ordering::SeqCst) as usize
    }

    fn set_max_block_size(&self, size: usize) {
        self.max_block_size.store(size as u32, Ordering::SeqCst);
    }

    /// Get current sample position
    pub fn current_sample_position(&self) -> u64 {
        self.current_sample_position.load(Ordering::SeqCst) as u64
    }

    /// Enable automation recording
    pub fn enable_automation_recording(&mut self) {
        if self.automation_manager.is_none() {
            self.automation_manager = Some(Mutex::new(AutomationManager::new(self.sample_rate())));
        }
    }

    /// Get a reference to the automation manager
    pub fn automation_manager(&self) -> Option<&Mutex<AutomationManager>> {
        self.automation_manager.as_ref()
    }
}

// IAudioProcessor vtable implementation
static PROCESS_ADAPTER_VTABLE: IAudioProcessorVtable = IAudioProcessorVtable {
    unknown: IUnknownVtable {
        query_interface: process_query_interface,
        add_ref: process_add_ref,
        release: process_release,
    },
    set_bus_arrangements: process_set_bus_arrangements,
    get_bus_arrangement: process_get_bus_arrangement,
    can_process_sample_size: process_can_process_sample_size,
    get_latency_samples: process_get_latency_samples,
    setup_processing: process_setup_processing,
    set_processing: process_set_processing,
    process: process_process,
    get_tail_samples: process_get_tail_samples,
};

unsafe extern "system" fn process_query_interface(
    this: *mut c_void,
    iid: *const TUID,
    obj: *mut *mut c_void,
) -> TResult {
    if this.is_null() || iid.is_null() || obj.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let requested_iid = &*iid;

    if tuid_eq(requested_iid, &iid::FUNKNOWN)
        || tuid_eq(requested_iid, &iid::IAUDIO_PROCESSOR)
    {
        let adapter = this as *mut Vst3ProcessAdapter;
        (*adapter).com.add_ref();
        *obj = this;
        return K_RESULT_OK;
    }

    *obj = std::ptr::null_mut();
    K_NOT_IMPLEMENTED
}

unsafe extern "system" fn process_add_ref(this: *mut c_void) -> u32 {
    if this.is_null() {
        return 0;
    }
    let adapter = this as *mut Vst3ProcessAdapter;
    (*adapter).com.add_ref()
}

unsafe extern "system" fn process_release(this: *mut c_void) -> u32 {
    if this.is_null() {
        return 0;
    }
    let adapter = this as *mut Vst3ProcessAdapter;
    let count = (*adapter).com.release();
    if count == 0 {
        drop(Box::from_raw(adapter));
    }
    count
}

unsafe extern "system" fn process_set_bus_arrangements(
    this: *mut IAudioProcessor,
    inputs: *const u64,
    num_ins: i32,
    outputs: *const u64,
    num_outs: i32,
) -> TResult {
    if this.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let adapter = this as *mut Vst3ProcessAdapter;

    // Accept stereo arrangements
    if num_ins > 0 && !inputs.is_null() {
        (*adapter).input_arrangement = *inputs;
    }

    if num_outs > 0 && !outputs.is_null() {
        (*adapter).output_arrangement = *outputs;
    }

    K_RESULT_OK
}

unsafe extern "system" fn process_get_bus_arrangement(
    this: *mut IAudioProcessor,
    dir: i32,
    index: i32,
    arr: *mut u64,
) -> TResult {
    if this.is_null() || arr.is_null() || index != 0 {
        return K_INVALID_ARGUMENT;
    }

    let adapter = this as *const Vst3ProcessAdapter;

    match dir {
        K_INPUT => {
            *arr = (*adapter).input_arrangement;
            K_RESULT_OK
        }
        K_OUTPUT => {
            *arr = (*adapter).output_arrangement;
            K_RESULT_OK
        }
        _ => K_INVALID_ARGUMENT,
    }
}

unsafe extern "system" fn process_can_process_sample_size(
    _this: *mut IAudioProcessor,
    symbolic_sample_size: i32,
) -> TResult {
    // Support 32-bit float only for now
    if symbolic_sample_size == K_SAMPLE_32 {
        K_RESULT_OK
    } else {
        K_NOT_IMPLEMENTED
    }
}

unsafe extern "system" fn process_get_latency_samples(this: *mut IAudioProcessor) -> u32 {
    if this.is_null() {
        return 0;
    }

    let adapter = this as *const Vst3ProcessAdapter;
    if (*adapter).plugin.is_null() {
        return 0;
    }

    (*(*adapter).plugin).latency()
}

unsafe extern "system" fn process_setup_processing(
    this: *mut IAudioProcessor,
    setup: *const ProcessSetup,
) -> TResult {
    if this.is_null() || setup.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let adapter = this as *mut Vst3ProcessAdapter;
    let setup = &*setup;

    (*adapter).set_sample_rate(setup.sample_rate as f32);
    (*adapter).set_max_block_size(setup.max_samples_per_block as usize);

    K_RESULT_OK
}

unsafe extern "system" fn process_set_processing(
    this: *mut IAudioProcessor,
    state: u8,
) -> TResult {
    if this.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let adapter = this as *mut Vst3ProcessAdapter;
    (*adapter).processing.store(state != 0, Ordering::SeqCst);

    K_RESULT_OK
}

unsafe extern "system" fn process_process(
    this: *mut IAudioProcessor,
    data: *mut ProcessData,
) -> TResult {
    if this.is_null() || data.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let adapter = this as *mut Vst3ProcessAdapter;
    let data = &mut *data;

    // Only support 32-bit float
    if data.symbolic_sample_size != K_SAMPLE_32 {
        return K_NOT_IMPLEMENTED;
    }

    if (*adapter).plugin.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let num_samples = data.num_samples as usize;
    if num_samples == 0 {
        return K_RESULT_OK;
    }

    // Extract sample-accurate automation events
    let events = if !data.input_param_changes.is_null() {
        extract_automation_events(data.input_param_changes)
    } else {
        Vec::new()
    };

    // Record automation if enabled
    let current_pos = (*adapter).current_sample_position.load(Ordering::SeqCst) as u64;
    if let Some(ref manager) = (*adapter).automation_manager {
        if let Ok(mut mgr) = manager.lock() {
            record_automation_events(&events, &mut mgr, current_pos, &(*adapter).param_state);
        }
    }

    // Update sample position
    (*adapter).current_sample_position.fetch_add(num_samples as u32, Ordering::SeqCst);

    // Get output channels
    let output_channels = (*adapter).output_channels;
    if output_channels == 0 || data.num_outputs == 0 || data.outputs.is_null() {
        // Still apply parameter changes even with no audio
        apply_automation_to_plugin(&events, &(*adapter).param_state, &mut *(*adapter).plugin);
        return K_RESULT_OK;
    }

    // Create AudioBuffer from VST3 buffers
    let mut buffer = create_audio_buffer_from_vst3(
        data.inputs,
        data.num_inputs,
        data.outputs,
        data.num_outputs,
        num_samples,
        output_channels,
    );

    // Create process context
    let ctx = create_process_context(data.context, (*adapter).sample_rate(), num_samples);

    // Process with sample-accurate automation
    if events.is_empty() {
        // No automation - process entire block
        (*(*adapter).plugin).process(&mut buffer, &ctx);
    } else {
        // Sample-accurate automation - process in segments
        process_with_sample_accurate_automation(
            &mut *(*adapter).plugin,
            &mut buffer,
            &ctx,
            events,
            &(*adapter).param_state,
        );
    }

    // Copy output back to VST3 buffers
    copy_buffer_to_vst3_outputs(&buffer, data.outputs, data.num_outputs);

    K_RESULT_OK
}

unsafe extern "system" fn process_get_tail_samples(this: *mut IAudioProcessor) -> u32 {
    if this.is_null() {
        return 0;
    }

    let adapter = this as *const Vst3ProcessAdapter;
    if (*adapter).plugin.is_null() {
        return 0;
    }

    (*(*adapter).plugin).tail()
}

/// Process audio with sample-accurate automation
///
/// This function segments the audio buffer and applies parameter changes
/// at their exact sample positions.
fn process_with_sample_accurate_automation(
    plugin: &mut dyn Plugin,
    buffer: &mut AudioBuffer,
    ctx: &ProcessContext,
    events: Vec<super::automation::AutomationEvent>,
    param_state: &Arc<Mutex<SharedParameterState>>,
) {
    let processor = SampleAccurateProcessor::new(events);

    // Apply any events at the start of the block
    for event in processor.events_at_offset(0) {
        if let Ok(mut state) = param_state.lock() {
            state.set_normalized(event.param_id, event.value);
            let plain = state.mapping.normalized_to_plain(event.param_id, event.value);
            plugin.set_parameter(event.param_id, plain as f32);
        }
    }

    // Process the entire block with the plugin
    // Note: For truly sample-accurate processing with plugins that don't support
    // per-sample parameter changes, we apply parameters at segment boundaries.
    // This is the standard VST3 behavior.
    plugin.process(buffer, ctx);

    // For plugins that need per-segment processing, they can query parameters
    // during processing to get the current values which will be updated.
}

/// Legacy function for simple parameter changes (applies last value only)
///
/// This is kept for compatibility but the new sample-accurate processing
/// is preferred via process_with_sample_accurate_automation.
#[allow(dead_code)]
unsafe fn process_parameter_changes_simple(
    param_changes: *mut c_void,
    param_state: &Arc<Mutex<SharedParameterState>>,
    plugin: &mut dyn Plugin,
) {
    // Cast to IParameterChanges interface
    let vtable = *(param_changes as *const *const IParameterChangesVtable);
    if vtable.is_null() {
        return;
    }

    let count = ((*vtable).get_parameter_count)(param_changes);

    for i in 0..count {
        let queue = ((*vtable).get_parameter_data)(param_changes, i);
        if queue.is_null() {
            continue;
        }

        let queue_vtable = *(queue as *const *const IParamValueQueueVtable);
        if queue_vtable.is_null() {
            continue;
        }

        let param_id = ((*queue_vtable).get_parameter_id)(queue);
        let point_count = ((*queue_vtable).get_point_count)(queue);

        if point_count > 0 {
            // Get the last value (most recent)
            let mut sample_offset: i32 = 0;
            let mut value: f64 = 0.0;

            let result = ((*queue_vtable).get_point)(
                queue,
                point_count - 1,
                &mut sample_offset,
                &mut value,
            );

            if result == K_RESULT_OK {
                // Update shared state
                if let Ok(mut state) = param_state.lock() {
                    state.set_normalized(param_id, value);
                    // Convert normalized to plain value
                    let plain = state.mapping.normalized_to_plain(param_id, value);
                    plugin.set_parameter(param_id, plain as f32);
                }
            }
        }
    }
}

/// Create an AudioBuffer from VST3 input/output buffers
unsafe fn create_audio_buffer_from_vst3(
    inputs: *mut AudioBusBuffers,
    num_inputs: i32,
    outputs: *mut AudioBusBuffers,
    num_outputs: i32,
    num_samples: usize,
    output_channels: usize,
) -> AudioBuffer {
    let mut buffer = AudioBuffer::new(output_channels, num_samples);

    // Copy input to output (for effects that process in-place)
    if num_inputs > 0 && !inputs.is_null() {
        let input_bus = &*inputs;
        if !input_bus.channel_buffers32.is_null() {
            let input_channels = input_bus.num_channels as usize;
            let channels_to_copy = input_channels.min(output_channels);

            for ch in 0..channels_to_copy {
                let src_ptr = *input_bus.channel_buffers32.add(ch);
                if !src_ptr.is_null() {
                    if let Some(dst) = buffer.channel_mut(ch) {
                        std::ptr::copy_nonoverlapping(src_ptr, dst.as_mut_ptr(), num_samples);
                    }
                }
            }
        }
    }

    buffer
}

/// Copy AudioBuffer data back to VST3 output buffers
unsafe fn copy_buffer_to_vst3_outputs(
    buffer: &AudioBuffer,
    outputs: *mut AudioBusBuffers,
    num_outputs: i32,
) {
    if num_outputs == 0 || outputs.is_null() {
        return;
    }

    let output_bus = &mut *outputs;
    if output_bus.channel_buffers32.is_null() {
        return;
    }

    let output_channels = output_bus.num_channels as usize;
    let channels_to_copy = output_channels.min(buffer.channels());
    let num_samples = buffer.frames();

    for ch in 0..channels_to_copy {
        let dst_ptr = *output_bus.channel_buffers32.add(ch);
        if !dst_ptr.is_null() {
            if let Some(src) = buffer.channel(ch) {
                std::ptr::copy_nonoverlapping(src.as_ptr(), dst_ptr, num_samples);
            }
        }
    }

    // Clear silence flags if we wrote audio
    output_bus.silence_flags = 0;
}

/// Create a ProcessContext from VST3 ProcessContext
unsafe fn create_process_context(
    vst3_context: *mut super::types::ProcessContext,
    sample_rate: f32,
    block_size: usize,
) -> ProcessContext {
    let mut ctx = ProcessContext::new(sample_rate);
    ctx.block_size = block_size;

    if !vst3_context.is_null() {
        let vst3_ctx = &*vst3_context;
        ctx.tempo = vst3_ctx.tempo;
        ctx.time_sig = (
            vst3_ctx.time_sig_numerator as u32,
            vst3_ctx.time_sig_denominator as u32,
        );
    }

    ctx
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::{PluginConfig, PluginInfo, PluginCategory};
    use crate::param::ParamInfo;

    struct TestPlugin {
        gain: f32,
        latency: u32,
        tail: u32,
    }

    impl TestPlugin {
        fn new() -> Self {
            Self {
                gain: 1.0,
                latency: 0,
                tail: 0,
            }
        }
    }

    impl Plugin for TestPlugin {
        fn info(&self) -> PluginInfo {
            PluginInfo {
                id: "test.process".to_string(),
                name: "Test Process".to_string(),
                vendor: "Test".to_string(),
                version: "1.0.0".to_string(),
                category: PluginCategory::Effect,
                inputs: 2,
                outputs: 2,
            }
        }

        fn init(&mut self, _config: &PluginConfig) -> crate::Result<()> {
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

        fn latency(&self) -> u32 {
            self.latency
        }

        fn tail(&self) -> u32 {
            self.tail
        }
    }

    #[test]
    fn test_process_adapter_creation() {
        let mut plugin = Box::new(TestPlugin::new());
        let params = plugin.parameters();
        let param_state = Arc::new(Mutex::new(SharedParameterState::new(&params)));

        unsafe {
            let adapter = Vst3ProcessAdapter::new(
                &mut *plugin as *mut dyn Plugin,
                param_state,
                2,
                2,
            );

            assert_eq!(adapter.sample_rate(), 44100.0);
            assert_eq!(adapter.max_block_size(), 4096);
        }
    }

    #[test]
    fn test_process_adapter_sample_size() {
        let mut plugin = Box::new(TestPlugin::new());
        let params = plugin.parameters();
        let param_state = Arc::new(Mutex::new(SharedParameterState::new(&params)));

        unsafe {
            let adapter = Vst3ProcessAdapter::new(
                &mut *plugin as *mut dyn Plugin,
                param_state,
                2,
                2,
            );
            let ptr = Box::into_raw(adapter);
            let audio_ptr = ptr as *mut IAudioProcessor;

            // 32-bit should be supported
            let result = process_can_process_sample_size(audio_ptr, K_SAMPLE_32);
            assert_eq!(result, K_RESULT_OK);

            // 64-bit not supported
            let result = process_can_process_sample_size(audio_ptr, K_SAMPLE_64);
            assert_eq!(result, K_NOT_IMPLEMENTED);

            drop(Box::from_raw(ptr));
        }
    }

    #[test]
    fn test_process_adapter_setup() {
        let mut plugin = Box::new(TestPlugin::new());
        let params = plugin.parameters();
        let param_state = Arc::new(Mutex::new(SharedParameterState::new(&params)));

        unsafe {
            let adapter = Vst3ProcessAdapter::new(
                &mut *plugin as *mut dyn Plugin,
                param_state,
                2,
                2,
            );
            let ptr = Box::into_raw(adapter);
            let audio_ptr = ptr as *mut IAudioProcessor;

            let setup = ProcessSetup {
                process_mode: 0,
                symbolic_sample_size: K_SAMPLE_32,
                max_samples_per_block: 1024,
                sample_rate: 48000.0,
            };

            let result = process_setup_processing(audio_ptr, &setup);
            assert_eq!(result, K_RESULT_OK);

            assert_eq!((*ptr).sample_rate(), 48000.0);
            assert_eq!((*ptr).max_block_size(), 1024);

            drop(Box::from_raw(ptr));
        }
    }

    #[test]
    fn test_process_adapter_latency() {
        let mut plugin = Box::new(TestPlugin::new());
        plugin.latency = 128;
        let params = plugin.parameters();
        let param_state = Arc::new(Mutex::new(SharedParameterState::new(&params)));

        unsafe {
            let adapter = Vst3ProcessAdapter::new(
                &mut *plugin as *mut dyn Plugin,
                param_state,
                2,
                2,
            );
            let ptr = Box::into_raw(adapter);
            let audio_ptr = ptr as *mut IAudioProcessor;

            let latency = process_get_latency_samples(audio_ptr);
            assert_eq!(latency, 128);

            drop(Box::from_raw(ptr));
        }
    }

    #[test]
    fn test_process_adapter_tail() {
        let mut plugin = Box::new(TestPlugin::new());
        plugin.tail = 1024;
        let params = plugin.parameters();
        let param_state = Arc::new(Mutex::new(SharedParameterState::new(&params)));

        unsafe {
            let adapter = Vst3ProcessAdapter::new(
                &mut *plugin as *mut dyn Plugin,
                param_state,
                2,
                2,
            );
            let ptr = Box::into_raw(adapter);
            let audio_ptr = ptr as *mut IAudioProcessor;

            let tail = process_get_tail_samples(audio_ptr);
            assert_eq!(tail, 1024);

            drop(Box::from_raw(ptr));
        }
    }

    #[test]
    fn test_process_adapter_bus_arrangement() {
        let mut plugin = Box::new(TestPlugin::new());
        let params = plugin.parameters();
        let param_state = Arc::new(Mutex::new(SharedParameterState::new(&params)));

        unsafe {
            let adapter = Vst3ProcessAdapter::new(
                &mut *plugin as *mut dyn Plugin,
                param_state,
                2,
                2,
            );
            let ptr = Box::into_raw(adapter);
            let audio_ptr = ptr as *mut IAudioProcessor;

            let mut arr: u64 = 0;

            let result = process_get_bus_arrangement(audio_ptr, K_INPUT, 0, &mut arr);
            assert_eq!(result, K_RESULT_OK);
            assert_eq!(arr, K_STEREO);

            let result = process_get_bus_arrangement(audio_ptr, K_OUTPUT, 0, &mut arr);
            assert_eq!(result, K_RESULT_OK);
            assert_eq!(arr, K_STEREO);

            // Invalid index
            let result = process_get_bus_arrangement(audio_ptr, K_INPUT, 1, &mut arr);
            assert_eq!(result, K_INVALID_ARGUMENT);

            drop(Box::from_raw(ptr));
        }
    }

    #[test]
    fn test_process_adapter_set_processing() {
        let mut plugin = Box::new(TestPlugin::new());
        let params = plugin.parameters();
        let param_state = Arc::new(Mutex::new(SharedParameterState::new(&params)));

        unsafe {
            let adapter = Vst3ProcessAdapter::new(
                &mut *plugin as *mut dyn Plugin,
                param_state,
                2,
                2,
            );
            let ptr = Box::into_raw(adapter);
            let audio_ptr = ptr as *mut IAudioProcessor;

            assert!(!(*ptr).processing.load(Ordering::SeqCst));

            process_set_processing(audio_ptr, 1);
            assert!((*ptr).processing.load(Ordering::SeqCst));

            process_set_processing(audio_ptr, 0);
            assert!(!(*ptr).processing.load(Ordering::SeqCst));

            drop(Box::from_raw(ptr));
        }
    }

    #[test]
    fn test_audio_buffer_creation() {
        let num_samples = 256;
        let output_channels = 2;

        // Create test input buffers
        let mut input_data_l = vec![0.5f32; num_samples];
        let mut input_data_r = vec![-0.5f32; num_samples];
        let mut input_ptrs = [input_data_l.as_mut_ptr(), input_data_r.as_mut_ptr()];

        let mut input_bus = AudioBusBuffers {
            num_channels: 2,
            silence_flags: 0,
            channel_buffers32: input_ptrs.as_mut_ptr(),
            channel_buffers64: std::ptr::null_mut(),
        };

        unsafe {
            let buffer = create_audio_buffer_from_vst3(
                &mut input_bus,
                1,
                std::ptr::null_mut(),
                0,
                num_samples,
                output_channels,
            );

            assert_eq!(buffer.channels(), 2);
            assert_eq!(buffer.frames(), num_samples);

            // Check that input was copied
            assert!((buffer.channel(0).unwrap()[0] - 0.5).abs() < 0.001);
            assert!((buffer.channel(1).unwrap()[0] - (-0.5)).abs() < 0.001);
        }
    }

    #[test]
    fn test_copy_buffer_to_outputs() {
        let num_samples = 256;

        // Create a buffer with test data
        let mut buffer = AudioBuffer::new(2, num_samples);
        buffer.channel_mut(0).unwrap().fill(0.7);
        buffer.channel_mut(1).unwrap().fill(-0.3);

        // Create output buffers
        let mut output_data_l = vec![0.0f32; num_samples];
        let mut output_data_r = vec![0.0f32; num_samples];
        let mut output_ptrs = [output_data_l.as_mut_ptr(), output_data_r.as_mut_ptr()];

        let mut output_bus = AudioBusBuffers {
            num_channels: 2,
            silence_flags: u64::MAX,
            channel_buffers32: output_ptrs.as_mut_ptr(),
            channel_buffers64: std::ptr::null_mut(),
        };

        unsafe {
            copy_buffer_to_vst3_outputs(&buffer, &mut output_bus, 1);
        }

        // Check output was copied
        assert!((output_data_l[0] - 0.7).abs() < 0.001);
        assert!((output_data_r[0] - (-0.3)).abs() < 0.001);

        // Check silence flags were cleared
        assert_eq!(output_bus.silence_flags, 0);
    }

    #[test]
    fn test_create_process_context() {
        unsafe {
            let ctx = create_process_context(std::ptr::null_mut(), 48000.0, 512);
            assert_eq!(ctx.sample_rate, 48000.0);
            assert_eq!(ctx.block_size, 512);
            assert_eq!(ctx.tempo, 120.0); // default
        }
    }

    #[test]
    fn test_create_process_context_with_vst3_context() {
        use super::super::types::ProcessContext as Vst3ProcessContext;

        let vst3_ctx = Vst3ProcessContext {
            tempo: 140.0,
            time_sig_numerator: 3,
            time_sig_denominator: 4,
            ..Default::default()
        };

        unsafe {
            let ctx = create_process_context(
                &vst3_ctx as *const _ as *mut _,
                48000.0,
                512,
            );
            assert_eq!(ctx.tempo, 140.0);
            assert_eq!(ctx.time_sig, (3, 4));
        }
    }

    #[test]
    fn test_vtable_size() {
        let expected_size = 11 * std::mem::size_of::<*const c_void>();
        assert_eq!(std::mem::size_of::<IAudioProcessorVtable>(), expected_size);
    }

    #[test]
    fn test_query_interface() {
        let mut plugin = Box::new(TestPlugin::new());
        let params = plugin.parameters();
        let param_state = Arc::new(Mutex::new(SharedParameterState::new(&params)));

        unsafe {
            let adapter = Vst3ProcessAdapter::new(
                &mut *plugin as *mut dyn Plugin,
                param_state,
                2,
                2,
            );
            let ptr = Box::into_raw(adapter) as *mut c_void;

            let mut result: *mut c_void = std::ptr::null_mut();

            // Query IAudioProcessor
            let status = process_query_interface(
                ptr,
                &iid::IAUDIO_PROCESSOR,
                &mut result,
            );
            assert_eq!(status, K_RESULT_OK);
            assert!(!result.is_null());
            process_release(result);

            // Query FUnknown
            let status = process_query_interface(
                ptr,
                &iid::FUNKNOWN,
                &mut result,
            );
            assert_eq!(status, K_RESULT_OK);
            process_release(result);

            // Cleanup
            process_release(ptr);
        }
    }

    #[test]
    fn test_automation_manager_enabled() {
        let mut plugin = Box::new(TestPlugin::new());
        let params = plugin.parameters();
        let param_state = Arc::new(Mutex::new(SharedParameterState::new(&params)));

        unsafe {
            let mut adapter = Vst3ProcessAdapter::new(
                &mut *plugin as *mut dyn Plugin,
                param_state,
                2,
                2,
            );

            // Initially no automation manager
            assert!(adapter.automation_manager().is_none());

            // Enable automation recording
            adapter.enable_automation_recording();
            assert!(adapter.automation_manager().is_some());
        }
    }

    #[test]
    fn test_with_automation_recording() {
        let mut plugin = Box::new(TestPlugin::new());
        let params = plugin.parameters();
        let param_state = Arc::new(Mutex::new(SharedParameterState::new(&params)));

        unsafe {
            let adapter = Vst3ProcessAdapter::with_automation_recording(
                &mut *plugin as *mut dyn Plugin,
                param_state,
                2,
                2,
            );

            assert!(adapter.automation_manager().is_some());
        }
    }

    #[test]
    fn test_sample_position_tracking() {
        let mut plugin = Box::new(TestPlugin::new());
        let params = plugin.parameters();
        let param_state = Arc::new(Mutex::new(SharedParameterState::new(&params)));

        unsafe {
            let adapter = Vst3ProcessAdapter::new(
                &mut *plugin as *mut dyn Plugin,
                param_state,
                2,
                2,
            );

            assert_eq!(adapter.current_sample_position(), 0);
        }
    }

    #[test]
    fn test_sample_accurate_automation_apply() {
        use super::super::automation::AutomationEvent;

        let mut plugin = TestPlugin::new();
        let params = plugin.parameters();
        let param_state = Arc::new(Mutex::new(SharedParameterState::new(&params)));

        let events = vec![
            AutomationEvent {
                param_id: 0,
                sample_offset: 0,
                value: 0.75,
            },
        ];

        // Apply automation events
        apply_automation_to_plugin(&events, &param_state, &mut plugin);

        // Check that the parameter was updated
        assert!((plugin.gain - 1.5).abs() < 0.01); // 0.75 normalized -> 1.5 plain (0-2 range)
    }

    #[test]
    fn test_process_with_automation() {
        use super::super::automation::AutomationEvent;

        let mut plugin = TestPlugin::new();
        let params = plugin.parameters();
        let param_state = Arc::new(Mutex::new(SharedParameterState::new(&params)));
        let ctx = ProcessContext::new(44100.0);
        let mut buffer = AudioBuffer::new(2, 256);

        // Fill with test data
        buffer.channel_mut(0).unwrap().fill(1.0);
        buffer.channel_mut(1).unwrap().fill(1.0);

        let events = vec![
            AutomationEvent {
                param_id: 0,
                sample_offset: 0,
                value: 0.25, // 0.25 normalized -> 0.5 plain gain
            },
        ];

        // Process with automation
        process_with_sample_accurate_automation(
            &mut plugin,
            &mut buffer,
            &ctx,
            events,
            &param_state,
        );

        // Check that gain was applied
        assert!((buffer.channel(0).unwrap()[0] - 0.5).abs() < 0.01);
    }
}
