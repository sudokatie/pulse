//! VST3 IUnitInfo implementation for preset support

use std::ffi::c_void;
use crate::preset::{Preset, PresetBank};

use super::com::{IUnknownVtable};
use super::types::{TResult, TUID, K_RESULT_OK, K_INVALID_ARGUMENT, K_NOT_IMPLEMENTED, iid, tuid_eq};

// Unit constants
pub const K_ROOT_UNIT_ID: i32 = 0;
pub const K_NO_PARENT_UNIT_ID: i32 = -1;
pub const K_NO_PROGRAM_LIST_ID: i32 = -1;

/// IUnitInfo vtable
#[repr(C)]
pub struct IUnitInfoVtable {
    pub unknown: IUnknownVtable,
    pub get_unit_count: unsafe extern "system" fn(this: *mut c_void) -> i32,
    pub get_unit_info: unsafe extern "system" fn(
        this: *mut c_void,
        unit_index: i32,
        info: *mut UnitInfo,
    ) -> TResult,
    pub get_program_list_count: unsafe extern "system" fn(this: *mut c_void) -> i32,
    pub get_program_list_info: unsafe extern "system" fn(
        this: *mut c_void,
        list_index: i32,
        info: *mut ProgramListInfo,
    ) -> TResult,
    pub get_program_name: unsafe extern "system" fn(
        this: *mut c_void,
        list_id: i32,
        program_index: i32,
        name: *mut [u16; 128],
    ) -> TResult,
    pub get_program_info: unsafe extern "system" fn(
        this: *mut c_void,
        list_id: i32,
        program_index: i32,
        attribute_id: *const i8,
        attribute_value: *mut [u16; 128],
    ) -> TResult,
    pub has_program_pitch_names: unsafe extern "system" fn(
        this: *mut c_void,
        list_id: i32,
        program_index: i32,
    ) -> TResult,
    pub get_program_pitch_name: unsafe extern "system" fn(
        this: *mut c_void,
        list_id: i32,
        program_index: i32,
        midi_pitch: i16,
        name: *mut [u16; 128],
    ) -> TResult,
    pub get_selected_unit: unsafe extern "system" fn(this: *mut c_void) -> i32,
    pub select_unit: unsafe extern "system" fn(
        this: *mut c_void,
        unit_id: i32,
    ) -> TResult,
    pub get_unit_by_bus: unsafe extern "system" fn(
        this: *mut c_void,
        media_type: i32,
        bus_direction: i32,
        bus_index: i32,
        channel: i32,
        unit_id: *mut i32,
    ) -> TResult,
    pub set_unit_program_data: unsafe extern "system" fn(
        this: *mut c_void,
        list_or_unit_id: i32,
        program_index: i32,
        data: *mut c_void,
    ) -> TResult,
}

/// Unit info structure
#[repr(C)]
#[derive(Debug, Clone)]
pub struct UnitInfo {
    pub id: i32,
    pub parent_unit_id: i32,
    pub name: [u16; 128],
    pub program_list_id: i32,
}

impl Default for UnitInfo {
    fn default() -> Self {
        Self {
            id: 0,
            parent_unit_id: K_NO_PARENT_UNIT_ID,
            name: [0; 128],
            program_list_id: K_NO_PROGRAM_LIST_ID,
        }
    }
}

impl UnitInfo {
    pub fn new(id: i32, name: &str, parent_id: i32, program_list_id: i32) -> Self {
        let mut info = Self {
            id,
            parent_unit_id: parent_id,
            name: [0; 128],
            program_list_id,
        };
        copy_str_to_u16(name, &mut info.name);
        info
    }
}

/// Program list info structure
#[repr(C)]
#[derive(Debug, Clone)]
pub struct ProgramListInfo {
    pub id: i32,
    pub name: [u16; 128],
    pub program_count: i32,
}

impl Default for ProgramListInfo {
    fn default() -> Self {
        Self {
            id: 0,
            name: [0; 128],
            program_count: 0,
        }
    }
}

impl ProgramListInfo {
    pub fn new(id: i32, name: &str, program_count: i32) -> Self {
        let mut info = Self {
            id,
            name: [0; 128],
            program_count,
        };
        copy_str_to_u16(name, &mut info.name);
        info
    }
}

/// Preset provider trait for abstraction over different preset sources
pub trait PresetProvider: Send + Sync {
    /// Get the number of preset banks (units)
    fn bank_count(&self) -> usize;

    /// Get bank info by index
    fn bank_info(&self, index: usize) -> Option<(&str, usize)>;

    /// Get preset name by bank index and preset index
    fn preset_name(&self, bank_index: usize, preset_index: usize) -> Option<&str>;

    /// Get preset by bank index and preset index
    fn preset(&self, bank_index: usize, preset_index: usize) -> Option<&Preset>;
}

/// Factory presets - compiled into the plugin
pub struct FactoryPresets {
    banks: Vec<PresetBank>,
}

impl FactoryPresets {
    pub fn new() -> Self {
        Self { banks: Vec::new() }
    }

    pub fn add_bank(&mut self, bank: PresetBank) {
        self.banks.push(bank);
    }

    pub fn with_bank(mut self, bank: PresetBank) -> Self {
        self.banks.push(bank);
        self
    }
}

impl Default for FactoryPresets {
    fn default() -> Self {
        Self::new()
    }
}

impl PresetProvider for FactoryPresets {
    fn bank_count(&self) -> usize {
        self.banks.len()
    }

    fn bank_info(&self, index: usize) -> Option<(&str, usize)> {
        self.banks.get(index).map(|b| (b.name.as_str(), b.count()))
    }

    fn preset_name(&self, bank_index: usize, preset_index: usize) -> Option<&str> {
        self.banks.get(bank_index)?.presets.get(preset_index).map(|p| p.name.as_str())
    }

    fn preset(&self, bank_index: usize, preset_index: usize) -> Option<&Preset> {
        self.banks.get(bank_index)?.presets.get(preset_index)
    }
}

/// VST3 Unit Info provider
pub struct Vst3UnitInfo {
    /// Factory presets (compiled in)
    factory_presets: FactoryPresets,
    /// Plugin ID for loading user presets
    plugin_id: String,
    /// Currently selected unit
    selected_unit: i32,
}

impl Vst3UnitInfo {
    pub fn new(plugin_id: impl Into<String>) -> Self {
        Self {
            factory_presets: FactoryPresets::new(),
            plugin_id: plugin_id.into(),
            selected_unit: K_ROOT_UNIT_ID,
        }
    }

    pub fn with_factory_presets(mut self, presets: FactoryPresets) -> Self {
        self.factory_presets = presets;
        self
    }

    /// Get total number of units (root unit + one per bank)
    pub fn unit_count(&self) -> i32 {
        // Always have at least root unit
        1 + self.factory_presets.bank_count() as i32
    }

    /// Get unit info by index
    pub fn get_unit_info(&self, index: i32) -> Option<UnitInfo> {
        if index == 0 {
            // Root unit
            Some(UnitInfo::new(
                K_ROOT_UNIT_ID,
                "Root",
                K_NO_PARENT_UNIT_ID,
                if self.factory_presets.bank_count() > 0 { 0 } else { K_NO_PROGRAM_LIST_ID },
            ))
        } else {
            let bank_index = (index - 1) as usize;
            let (name, _) = self.factory_presets.bank_info(bank_index)?;
            Some(UnitInfo::new(
                index,
                name,
                K_ROOT_UNIT_ID,
                index, // Program list ID matches unit ID
            ))
        }
    }

    /// Get number of program lists
    pub fn program_list_count(&self) -> i32 {
        self.factory_presets.bank_count() as i32
    }

    /// Get program list info by index
    pub fn get_program_list_info(&self, index: i32) -> Option<ProgramListInfo> {
        let bank_index = index as usize;
        let (name, count) = self.factory_presets.bank_info(bank_index)?;
        Some(ProgramListInfo::new(index, name, count as i32))
    }

    /// Get program name
    pub fn get_program_name(&self, list_id: i32, program_index: i32) -> Option<&str> {
        self.factory_presets.preset_name(list_id as usize, program_index as usize)
    }

    /// Get preset
    pub fn get_preset(&self, list_id: i32, program_index: i32) -> Option<&Preset> {
        self.factory_presets.preset(list_id as usize, program_index as usize)
    }

    /// Get selected unit
    pub fn selected_unit(&self) -> i32 {
        self.selected_unit
    }

    /// Select a unit
    pub fn select_unit(&mut self, unit_id: i32) -> bool {
        if unit_id >= 0 && unit_id < self.unit_count() {
            self.selected_unit = unit_id;
            true
        } else {
            false
        }
    }
}

// IUnitInfo vtable implementation
pub static UNIT_INFO_VTABLE: IUnitInfoVtable = IUnitInfoVtable {
    unknown: IUnknownVtable {
        query_interface: unit_info_query_interface,
        add_ref: unit_info_add_ref,
        release: unit_info_release,
    },
    get_unit_count: unit_info_get_unit_count,
    get_unit_info: unit_info_get_unit_info,
    get_program_list_count: unit_info_get_program_list_count,
    get_program_list_info: unit_info_get_program_list_info,
    get_program_name: unit_info_get_program_name,
    get_program_info: unit_info_get_program_info,
    has_program_pitch_names: unit_info_has_program_pitch_names,
    get_program_pitch_name: unit_info_get_program_pitch_name,
    get_selected_unit: unit_info_get_selected_unit,
    select_unit: unit_info_select_unit,
    get_unit_by_bus: unit_info_get_unit_by_bus,
    set_unit_program_data: unit_info_set_unit_program_data,
};

unsafe extern "system" fn unit_info_query_interface(
    this: *mut c_void,
    riid: *const TUID,
    obj: *mut *mut c_void,
) -> TResult {
    if this.is_null() || riid.is_null() || obj.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let requested_iid = &*riid;

    if tuid_eq(requested_iid, &iid::FUNKNOWN) || tuid_eq(requested_iid, &iid::IUNIT_INFO) {
        *obj = this;
        return K_RESULT_OK;
    }

    *obj = std::ptr::null_mut();
    K_NOT_IMPLEMENTED
}

unsafe extern "system" fn unit_info_add_ref(_this: *mut c_void) -> u32 {
    1
}

unsafe extern "system" fn unit_info_release(_this: *mut c_void) -> u32 {
    1
}

unsafe extern "system" fn unit_info_get_unit_count(this: *mut c_void) -> i32 {
    if this.is_null() {
        return 0;
    }

    let info = &*(this as *const Vst3UnitInfo);
    info.unit_count()
}

unsafe extern "system" fn unit_info_get_unit_info(
    this: *mut c_void,
    unit_index: i32,
    info: *mut UnitInfo,
) -> TResult {
    if this.is_null() || info.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let unit_info = &*(this as *const Vst3UnitInfo);

    match unit_info.get_unit_info(unit_index) {
        Some(unit) => {
            *info = unit;
            K_RESULT_OK
        }
        None => K_INVALID_ARGUMENT,
    }
}

unsafe extern "system" fn unit_info_get_program_list_count(this: *mut c_void) -> i32 {
    if this.is_null() {
        return 0;
    }

    let info = &*(this as *const Vst3UnitInfo);
    info.program_list_count()
}

unsafe extern "system" fn unit_info_get_program_list_info(
    this: *mut c_void,
    list_index: i32,
    info: *mut ProgramListInfo,
) -> TResult {
    if this.is_null() || info.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let unit_info = &*(this as *const Vst3UnitInfo);

    match unit_info.get_program_list_info(list_index) {
        Some(list) => {
            *info = list;
            K_RESULT_OK
        }
        None => K_INVALID_ARGUMENT,
    }
}

unsafe extern "system" fn unit_info_get_program_name(
    this: *mut c_void,
    list_id: i32,
    program_index: i32,
    name: *mut [u16; 128],
) -> TResult {
    if this.is_null() || name.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let unit_info = &*(this as *const Vst3UnitInfo);

    match unit_info.get_program_name(list_id, program_index) {
        Some(preset_name) => {
            copy_str_to_u16(preset_name, &mut *name);
            K_RESULT_OK
        }
        None => K_INVALID_ARGUMENT,
    }
}

unsafe extern "system" fn unit_info_get_program_info(
    _this: *mut c_void,
    _list_id: i32,
    _program_index: i32,
    _attribute_id: *const i8,
    _attribute_value: *mut [u16; 128],
) -> TResult {
    // Optional: return program attributes (category, designer, etc.)
    K_NOT_IMPLEMENTED
}

unsafe extern "system" fn unit_info_has_program_pitch_names(
    _this: *mut c_void,
    _list_id: i32,
    _program_index: i32,
) -> TResult {
    // We don't support pitch names
    K_NOT_IMPLEMENTED
}

unsafe extern "system" fn unit_info_get_program_pitch_name(
    _this: *mut c_void,
    _list_id: i32,
    _program_index: i32,
    _midi_pitch: i16,
    _name: *mut [u16; 128],
) -> TResult {
    K_NOT_IMPLEMENTED
}

unsafe extern "system" fn unit_info_get_selected_unit(this: *mut c_void) -> i32 {
    if this.is_null() {
        return K_ROOT_UNIT_ID;
    }

    let info = &*(this as *const Vst3UnitInfo);
    info.selected_unit()
}

unsafe extern "system" fn unit_info_select_unit(this: *mut c_void, unit_id: i32) -> TResult {
    if this.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let info = &mut *(this as *mut Vst3UnitInfo);

    if info.select_unit(unit_id) {
        K_RESULT_OK
    } else {
        K_INVALID_ARGUMENT
    }
}

unsafe extern "system" fn unit_info_get_unit_by_bus(
    _this: *mut c_void,
    _media_type: i32,
    _bus_direction: i32,
    _bus_index: i32,
    _channel: i32,
    unit_id: *mut i32,
) -> TResult {
    if unit_id.is_null() {
        return K_INVALID_ARGUMENT;
    }

    // All buses belong to root unit
    *unit_id = K_ROOT_UNIT_ID;
    K_RESULT_OK
}

unsafe extern "system" fn unit_info_set_unit_program_data(
    _this: *mut c_void,
    _list_or_unit_id: i32,
    _program_index: i32,
    _data: *mut c_void,
) -> TResult {
    // Not supported for now
    K_NOT_IMPLEMENTED
}

fn copy_str_to_u16(src: &str, dst: &mut [u16]) {
    let len = src.chars().count().min(dst.len() - 1);
    for (i, c) in src.chars().take(len).enumerate() {
        dst[i] = c as u16;
    }
    dst[len] = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_presets() -> FactoryPresets {
        let mut factory = PresetBank::new("Factory", "test.plugin");
        let mut preset1 = Preset::new("test.plugin", "Init");
        preset1.set_param("volume", 0.5);
        factory.add(preset1);

        let mut preset2 = Preset::new("test.plugin", "Lead");
        preset2.set_param("volume", 0.8);
        factory.add(preset2);

        FactoryPresets::new().with_bank(factory)
    }

    #[test]
    fn test_factory_presets() {
        let presets = create_test_presets();

        assert_eq!(presets.bank_count(), 1);
        assert_eq!(presets.bank_info(0), Some(("Factory", 2)));
        assert_eq!(presets.preset_name(0, 0), Some("Init"));
        assert_eq!(presets.preset_name(0, 1), Some("Lead"));
        assert_eq!(presets.preset_name(0, 2), None);
    }

    #[test]
    fn test_unit_info_creation() {
        let presets = create_test_presets();
        let unit_info = Vst3UnitInfo::new("test.plugin").with_factory_presets(presets);

        // Root unit + 1 bank = 2 units
        assert_eq!(unit_info.unit_count(), 2);
        assert_eq!(unit_info.program_list_count(), 1);
    }

    #[test]
    fn test_unit_info_get_unit() {
        let presets = create_test_presets();
        let unit_info = Vst3UnitInfo::new("test.plugin").with_factory_presets(presets);

        // Root unit
        let root = unit_info.get_unit_info(0).unwrap();
        assert_eq!(root.id, K_ROOT_UNIT_ID);
        assert_eq!(root.parent_unit_id, K_NO_PARENT_UNIT_ID);

        // Bank unit
        let bank = unit_info.get_unit_info(1).unwrap();
        assert_eq!(bank.id, 1);
        assert_eq!(bank.parent_unit_id, K_ROOT_UNIT_ID);

        // Invalid index
        assert!(unit_info.get_unit_info(5).is_none());
    }

    #[test]
    fn test_program_list_info() {
        let presets = create_test_presets();
        let unit_info = Vst3UnitInfo::new("test.plugin").with_factory_presets(presets);

        let list = unit_info.get_program_list_info(0).unwrap();
        assert_eq!(list.id, 0);
        assert_eq!(list.program_count, 2);

        assert!(unit_info.get_program_list_info(1).is_none());
    }

    #[test]
    fn test_get_program_name() {
        let presets = create_test_presets();
        let unit_info = Vst3UnitInfo::new("test.plugin").with_factory_presets(presets);

        assert_eq!(unit_info.get_program_name(0, 0), Some("Init"));
        assert_eq!(unit_info.get_program_name(0, 1), Some("Lead"));
        assert_eq!(unit_info.get_program_name(0, 2), None);
        assert_eq!(unit_info.get_program_name(1, 0), None);
    }

    #[test]
    fn test_select_unit() {
        let presets = create_test_presets();
        let mut unit_info = Vst3UnitInfo::new("test.plugin").with_factory_presets(presets);

        assert_eq!(unit_info.selected_unit(), K_ROOT_UNIT_ID);

        assert!(unit_info.select_unit(1));
        assert_eq!(unit_info.selected_unit(), 1);

        // Invalid unit
        assert!(!unit_info.select_unit(100));
        assert_eq!(unit_info.selected_unit(), 1); // Unchanged
    }

    #[test]
    fn test_unit_info_struct() {
        let info = UnitInfo::new(1, "Test Unit", K_ROOT_UNIT_ID, 0);
        assert_eq!(info.id, 1);
        assert_eq!(info.parent_unit_id, K_ROOT_UNIT_ID);
        assert_eq!(info.program_list_id, 0);

        // Check name
        let len = info.name.iter().position(|&c| c == 0).unwrap_or(128);
        let name = String::from_utf16_lossy(&info.name[..len]);
        assert_eq!(name, "Test Unit");
    }

    #[test]
    fn test_program_list_info_struct() {
        let info = ProgramListInfo::new(0, "Factory Presets", 10);
        assert_eq!(info.id, 0);
        assert_eq!(info.program_count, 10);

        let len = info.name.iter().position(|&c| c == 0).unwrap_or(128);
        let name = String::from_utf16_lossy(&info.name[..len]);
        assert_eq!(name, "Factory Presets");
    }

    #[test]
    fn test_get_preset() {
        let presets = create_test_presets();
        let unit_info = Vst3UnitInfo::new("test.plugin").with_factory_presets(presets);

        let preset = unit_info.get_preset(0, 0).unwrap();
        assert_eq!(preset.name, "Init");
        assert_eq!(preset.get_param("volume"), Some(0.5));

        let preset = unit_info.get_preset(0, 1).unwrap();
        assert_eq!(preset.name, "Lead");
        assert_eq!(preset.get_param("volume"), Some(0.8));
    }

    #[test]
    fn test_empty_unit_info() {
        let unit_info = Vst3UnitInfo::new("test.plugin");

        // Still has root unit
        assert_eq!(unit_info.unit_count(), 1);
        assert_eq!(unit_info.program_list_count(), 0);

        let root = unit_info.get_unit_info(0).unwrap();
        assert_eq!(root.id, K_ROOT_UNIT_ID);
        assert_eq!(root.program_list_id, K_NO_PROGRAM_LIST_ID);
    }

    #[test]
    fn test_vtable_functions() {
        let presets = create_test_presets();
        let mut unit_info = Vst3UnitInfo::new("test.plugin").with_factory_presets(presets);
        let ptr = &mut unit_info as *mut Vst3UnitInfo as *mut c_void;

        unsafe {
            // Test unit count
            let count = unit_info_get_unit_count(ptr);
            assert_eq!(count, 2);

            // Test get unit info
            let mut info = UnitInfo::default();
            let result = unit_info_get_unit_info(ptr, 0, &mut info);
            assert_eq!(result, K_RESULT_OK);
            assert_eq!(info.id, K_ROOT_UNIT_ID);

            // Test program list count
            let count = unit_info_get_program_list_count(ptr);
            assert_eq!(count, 1);

            // Test get program name
            let mut name: [u16; 128] = [0; 128];
            let result = unit_info_get_program_name(ptr, 0, 0, &mut name);
            assert_eq!(result, K_RESULT_OK);
            let len = name.iter().position(|&c| c == 0).unwrap_or(128);
            let name_str = String::from_utf16_lossy(&name[..len]);
            assert_eq!(name_str, "Init");

            // Test select unit
            let result = unit_info_select_unit(ptr, 1);
            assert_eq!(result, K_RESULT_OK);
            assert_eq!(unit_info_get_selected_unit(ptr), 1);
        }
    }
}
