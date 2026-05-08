use crate::catalog::BattleMasterData;

use super::DamageInput;

#[derive(Debug, thiserror::Error)]
pub enum DamageCalcError {
    #[error("Attacker not found")]
    AttackerNotFound,
    #[error("Defender not found")]
    DefenderNotFound,
    #[error("Move not found")]
    MoveNotFound,
}

fn calc_stat(base: u32, ap: u32, nature_mult: f64, is_hp: bool) -> u32 {
    if is_hp {
        base + 75 + ap
    } else {
        (((base + 20 + ap) as f64) * nature_mult).floor() as u32
    }
}

fn get_rank_mult(stage: i8) -> (u32, u32) {
    let s = stage.clamp(-6, 6);
    if s >= 0 {
        (2 + s as u32, 2)
    } else {
        (2, 2 + s.unsigned_abs() as u32)
    }
}

pub fn calculate_damage(
    master: &BattleMasterData,
    input: &DamageInput,
) -> Result<u32, DamageCalcError> {
    let atk_base = master
        .pokemon_stats
        .get(&input.attacker_id)
        .ok_or(DamageCalcError::AttackerNotFound)?;
    let def_base = master
        .pokemon_stats
        .get(&input.defender_id)
        .ok_or(DamageCalcError::DefenderNotFound)?;
    let m = master
        .moves
        .get(&input.move_id)
        .ok_or(DamageCalcError::MoveNotFound)?;

    if m.damage_class_id == 1 || m.power.unwrap_or(0) == 0 {
        return Ok(0);
    }
    let power = m.power.unwrap();

    let (a_idx, d_idx) = if m.damage_class_id == 2 {
        (1, 2)
    } else {
        (3, 4)
    };

    let get_nature_mult = |nature_id: u32, stat_idx: usize| -> f64 {
        if let Some(nature) = master.natures.get(&nature_id) {
            if nature.increased_stat_id == (stat_idx + 1) as u32 {
                return 1.1;
            }
            if nature.decreased_stat_id == (stat_idx + 1) as u32 {
                return 0.9;
            }
        }
        1.0
    };

    let a_nature = get_nature_mult(input.attacker_nature_id, a_idx);
    let d_nature = get_nature_mult(input.defender_nature_id, d_idx);

    let a_val = calc_stat(atk_base[a_idx], input.attacker_ap[a_idx], a_nature, false);
    let d_val = calc_stat(def_base[d_idx], input.defender_ap[d_idx], d_nature, false);

    let a_r = get_rank_mult(input.attacker_stages[a_idx]);
    let d_r = get_rank_mult(input.defender_stages[d_idx]);

    let final_a = if input.is_critical && input.attacker_stages[a_idx] < 0 {
        a_val
    } else {
        (a_val * a_r.0) / a_r.1
    };
    let final_d = if input.is_critical && input.defender_stages[d_idx] > 0 {
        d_val
    } else {
        (d_val * d_r.0) / d_r.1
    };

    let mut damage = (((22 * power * final_a) / final_d) / 50 + 2) as f64;

    if let Some(atk_types) = master.pokemon_types.get(&input.attacker_id)
        && atk_types.contains(&m.type_id)
    {
        damage = (damage * 1.5).floor();
    }

    damage = (damage * input.rng_roll).floor();
    if input.is_critical {
        damage = (damage * 1.5).floor();
    }

    if let Some(target_types) = master.pokemon_types.get(&input.defender_id) {
        let mut efficacy = 1.0;
        for &t_id in target_types {
            if let Some(&factor) = master.type_efficacy.get(&(m.type_id, t_id)) {
                efficacy *= factor as f64 / 100.0;
            }
        }
        damage = (damage * efficacy).floor();
    }

    Ok(damage as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{BattleMasterData, MoveData, NatureData};
    use std::collections::HashMap;

    fn fixture_master() -> BattleMasterData {
        let mut pokemon_stats = HashMap::new();
        pokemon_stats.insert(1, [100_u32, 130, 80, 60, 70, 110]);
        pokemon_stats.insert(2, [100, 80, 120, 60, 80, 60]);

        let mut moves = HashMap::new();
        moves.insert(
            10,
            MoveData {
                id: 10,
                type_id: 1,
                power: Some(80),
                damage_class_id: 2,
            },
        );
        moves.insert(
            20,
            MoveData {
                id: 20,
                type_id: 1,
                power: Some(0),
                damage_class_id: 1,
            },
        );
        moves.insert(
            30,
            MoveData {
                id: 30,
                type_id: 2,
                power: Some(90),
                damage_class_id: 3,
            },
        );
        moves.insert(
            40,
            MoveData {
                id: 40,
                type_id: 3,
                power: Some(100),
                damage_class_id: 2,
            },
        );

        let mut natures = HashMap::new();
        natures.insert(
            1,
            NatureData {
                id: 1,
                increased_stat_id: 0,
                decreased_stat_id: 0,
            },
        );
        natures.insert(
            2,
            NatureData {
                id: 2,
                increased_stat_id: 2,
                decreased_stat_id: 5,
            },
        );

        let mut type_efficacy = HashMap::new();
        type_efficacy.insert((1, 1), 50);
        type_efficacy.insert((1, 2), 200);
        type_efficacy.insert((3, 2), 0);

        let mut pokemon_types = HashMap::new();
        pokemon_types.insert(1, vec![1]);
        pokemon_types.insert(2, vec![2]);

        BattleMasterData {
            pokemon_stats,
            moves,
            natures,
            type_efficacy,
            pokemon_types,
        }
    }

    fn default_input(attacker: u32, defender: u32, move_id: u32) -> DamageInput {
        DamageInput {
            attacker_id: attacker,
            defender_id: defender,
            move_id,
            attacker_ap: [0; 6],
            defender_ap: [0; 6],
            attacker_nature_id: 1,
            defender_nature_id: 1,
            attacker_stages: [0; 8],
            defender_stages: [0; 8],
            is_critical: false,
            rng_roll: 1.0,
        }
    }

    #[test]
    fn status_move_deals_zero_damage() {
        let master = fixture_master();
        let input = default_input(1, 2, 20);
        assert_eq!(calculate_damage(&master, &input).unwrap(), 0);
    }

    #[test]
    fn physical_move_basic() {
        let master = fixture_master();
        let input = default_input(1, 2, 10);
        // A=150, D=140, base=(22*80*150)/140/50+2=39, STAB=58, eff*2=116
        assert_eq!(calculate_damage(&master, &input).unwrap(), 116);
    }

    #[test]
    fn type_immunity_deals_zero() {
        let master = fixture_master();
        let input = default_input(1, 2, 40);
        assert_eq!(calculate_damage(&master, &input).unwrap(), 0);
    }

    #[test]
    fn critical_hit_multiplier() {
        let master = fixture_master();
        let mut input = default_input(1, 2, 10);
        input.is_critical = true;
        // STAB=58, rng=58, crit=87, eff=174
        assert_eq!(calculate_damage(&master, &input).unwrap(), 174);
    }

    #[test]
    fn rng_roll_reduces_damage() {
        let master = fixture_master();
        let mut input = default_input(1, 2, 10);
        input.rng_roll = 0.85;
        // STAB=58, rng=floor(58*0.85)=49, eff=98
        assert_eq!(calculate_damage(&master, &input).unwrap(), 98);
    }

    #[test]
    fn rank_atk_plus_6() {
        let master = fixture_master();
        let mut input = default_input(1, 2, 10);
        input.attacker_stages[1] = 6;
        // final_a=150*8/2=600, base=(22*80*600)/140/50+2=152, STAB=228, eff=456
        assert_eq!(calculate_damage(&master, &input).unwrap(), 456);
    }

    #[test]
    fn rank_def_plus_6() {
        let master = fixture_master();
        let mut input = default_input(1, 2, 10);
        input.defender_stages[2] = 6;
        // final_d=140*8/2=560, base=(22*80*150)/560/50+2=11, STAB=floor(11*1.5)=16, eff=32
        assert_eq!(calculate_damage(&master, &input).unwrap(), 32);
    }

    #[test]
    fn rank_atk_minus_6() {
        let master = fixture_master();
        let mut input = default_input(1, 2, 10);
        input.attacker_stages[1] = -6;
        // final_a=150*2/8=37, base=(22*80*37)/140/50+2=11, STAB=floor(11*1.5)=16, eff=32
        assert_eq!(calculate_damage(&master, &input).unwrap(), 32);
    }

    #[test]
    fn crit_ignores_atk_debuff() {
        let master = fixture_master();
        let mut input = default_input(1, 2, 10);
        input.attacker_stages[1] = -6;
        input.is_critical = true;
        // crit + negative atk stage → final_a=a_val=150 (stage ignored)
        // base=39, STAB=58, rng=58, crit=87, eff=174
        assert_eq!(calculate_damage(&master, &input).unwrap(), 174);
    }

    #[test]
    fn crit_ignores_def_buff() {
        let master = fixture_master();
        let mut input = default_input(1, 2, 10);
        input.defender_stages[2] = 6;
        input.is_critical = true;
        // crit + positive def stage → final_d=d_val=140 (stage ignored)
        // base=39, STAB=58, rng=58, crit=87, eff=174
        assert_eq!(calculate_damage(&master, &input).unwrap(), 174);
    }

    #[test]
    fn nature_boost_increases_damage() {
        let master = fixture_master();
        let mut input = default_input(1, 2, 10);
        input.attacker_nature_id = 2;
        // A=(130+20)*1.1=165, base=(22*80*165)/140/50+2=43, STAB=floor(43*1.5)=64, eff=128
        assert_eq!(calculate_damage(&master, &input).unwrap(), 128);
    }

    #[test]
    fn attacker_not_found_error() {
        let master = fixture_master();
        let input = default_input(999, 2, 10);
        assert!(matches!(
            calculate_damage(&master, &input),
            Err(DamageCalcError::AttackerNotFound)
        ));
    }

    #[test]
    fn defender_not_found_error() {
        let master = fixture_master();
        let input = default_input(1, 999, 10);
        assert!(matches!(
            calculate_damage(&master, &input),
            Err(DamageCalcError::DefenderNotFound)
        ));
    }

    #[test]
    fn move_not_found_error() {
        let master = fixture_master();
        let input = default_input(1, 2, 999);
        assert!(matches!(
            calculate_damage(&master, &input),
            Err(DamageCalcError::MoveNotFound)
        ));
    }
}
