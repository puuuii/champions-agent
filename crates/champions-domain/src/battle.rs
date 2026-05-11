mod damage_formula;
mod damage_input;

pub use damage_formula::{
    DamageCalcError, calculate_damage, calculate_damage_with_stats, resolve_stat_value,
};
pub use damage_input::DamageInput;
