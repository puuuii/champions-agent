use crate::domain::damage::{DamageArgs, MasterData};

pub trait DamageCalculator {
    fn calculate(&self, master: &MasterData, args: &DamageArgs) -> Result<u32, String>;
}
