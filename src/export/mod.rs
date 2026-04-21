//! VST3 plugin export - compile Pulse plugins as VST3 bundles

pub mod adapter;
pub mod audio_processor;
pub mod automation;
pub mod bundle;
pub mod category;
pub mod com;
pub mod component;
pub mod controller;
pub mod edit_controller;
pub mod entry;
pub mod factory;
pub mod param_map;
pub mod process_adapter;
pub mod state_bridge;
pub mod types;
pub mod unit_info;

// Re-export the vst3_plugin attribute macro
pub use pulse_vst3_macro::vst3_plugin;

pub use adapter::{Vst3Adapter, SharedParameterState};
pub use audio_processor::IAudioProcessorVtable;
pub use bundle::{BundleBuilder, Platform, ValidationResult, default_install_path};
pub use category::category_to_vst3;
pub use com::{IUnknown, IUnknownVtable, ComRef};
pub use component::IComponentVtable;
pub use controller::Vst3EditController;
pub use edit_controller::IEditControllerVtable;
pub use entry::{create_entry_point_factory, PluginCreateFn, PluginRegistry};
pub use factory::{
    IPluginFactoryVtable, IPluginFactory2Vtable, IPluginFactory3Vtable,
    Vst3PluginFactory, FactoryInfo, generate_tuid,
};
pub use param_map::Vst3ParameterMapping;
pub use process_adapter::Vst3ProcessAdapter;
pub use state_bridge::MemoryStream;
pub use types::*;
pub use automation::{AutomationEvent, SampleAccurateProcessor, extract_automation_events, apply_automation_to_plugin, record_automation_events};
pub use unit_info::{Vst3UnitInfo, FactoryPresets, UnitInfo, ProgramListInfo, PresetProvider, IUnitInfoVtable, UNIT_INFO_VTABLE};
