//! Reverb effect - placeholder
pub struct Reverb;
impl Reverb {
    pub fn new(_sample_rate: u32) -> Self { Self }
    pub fn set_room_size(&mut self, _: f32) {}
    pub fn set_damping(&mut self, _: f32) {}
    pub fn set_wet(&mut self, _: f32) {}
}
