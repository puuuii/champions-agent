use crate::domain::damage::{DamageArgs, MasterData};
use crate::domain::damage_calc::DamageCalculator;

pub struct ChampionsDamageCalculator;

impl DamageCalculator for ChampionsDamageCalculator {
    fn calculate(&self, master: &MasterData, args: &DamageArgs) -> Result<u32, String> {
        // 既存の src/damage/calc.rs のロジックをここに移植
        Ok(0) // 仮実装
    }
}
