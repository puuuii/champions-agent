pub use crate::domain::damage_calc::DamageCalculator;

pub trait PartyIdentifierService {
    fn identify(&self, frame_data: &[u8]) -> Result<Vec<(String, f32)>, String>;
}
