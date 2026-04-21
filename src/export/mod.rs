//! VST3 plugin export - compile Pulse plugins as VST3 bundles

pub mod adapter;
pub mod audio_processor;
pub mod category;
pub mod com;
pub mod component;
pub mod controller;
pub mod edit_controller;
pub mod factory;
pub mod param_map;
pub mod process_adapter;
pub mod state_bridge;
pub mod types;

pub use adapter::{Vst3Adapter, SharedParameterState};
pub use audio_processor::IAudioProcessorVtable;
pub use category::category_to_vst3;
pub use com::{IUnknown, IUnknownVtable, ComRef};
pub use component::IComponentVtable;
pub use controller::Vst3EditController;
pub use edit_controller::IEditControllerVtable;
pub use factory::{IPluginFactoryVtable, IPluginFactory2Vtable, IPluginFactory3Vtable};
pub use param_map::Vst3ParameterMapping;
pub use process_adapter::Vst3ProcessAdapter;
pub use state_bridge::MemoryStream;
pub use types::*;
