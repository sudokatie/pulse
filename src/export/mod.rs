//! VST3 plugin export - compile Pulse plugins as VST3 bundles

pub mod audio_processor;
pub mod category;
pub mod com;
pub mod component;
pub mod edit_controller;
pub mod factory;
pub mod param_map;
pub mod types;

pub use audio_processor::IAudioProcessorVtable;
pub use category::category_to_vst3;
pub use com::{IUnknown, IUnknownVtable, ComRef};
pub use component::IComponentVtable;
pub use edit_controller::IEditControllerVtable;
pub use factory::{IPluginFactoryVtable, IPluginFactory2Vtable, IPluginFactory3Vtable};
pub use param_map::Vst3ParameterMapping;
pub use types::*;
