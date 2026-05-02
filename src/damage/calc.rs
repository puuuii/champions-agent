use super::models::{DamageArgs, MasterData};

/// ステータス算出 (Champions式)
fn calc_stat(base: u32, ap: u32, nature_mult: f64, is_hp: bool) -> u32 {
    if is_hp {
        base + 75 + ap
    } else {
        (((base + 20 + ap) as f64) * nature_mult).floor() as u32
    }
}

/// ランク倍率算出 (分子/分母)
fn get_rank_mult(stage: i8) -> (u32, u32) {
    let s = stage.clamp(-6, 6);
    if s >= 0 {
        (2 + s as u32, 2)
    } else {
        (2, 2 + s.unsigned_abs() as u32)
    }
}

pub fn calculate_damage(master: &MasterData, args: &DamageArgs) -> Result<u32, String> {
    let atk_base = master
        .pokemon_stats
        .get(&args.attacker_id)
        .ok_or("Attacker not found")?;
    let def_base = master
        .pokemon_stats
        .get(&args.defender_id)
        .ok_or("Defender not found")?;
    let m = master.moves.get(&args.move_id).ok_or("Move not found")?;

    // 変化技(1)または威力が0の場合はダメージ0[cite: 20, 26]
    if m.damage_class_id == 1 || m.power.unwrap_or(0) == 0 {
        return Ok(0);
    }
    let power = m.power.unwrap();

    // 物理(2)か特殊(3)かでインデックスを決定[cite: 18, 26]
    let (a_idx, d_idx) = if m.damage_class_id == 2 {
        (1, 2)
    } else {
        (3, 4)
    };

    // 性格補正の解決[cite: 20, 26]
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

    let a_nature = get_nature_mult(args.attacker_nature_id, a_idx);
    let d_nature = get_nature_mult(args.defender_nature_id, d_idx);

    let a_val = calc_stat(atk_base[a_idx], args.attacker_ap[a_idx], a_nature, false);
    let d_val = calc_stat(def_base[d_idx], args.defender_ap[d_idx], d_nature, false);

    let a_r = get_rank_mult(args.attacker_stages[a_idx]);
    let d_r = get_rank_mult(args.defender_stages[d_idx]);

    let final_a = if args.is_critical && args.attacker_stages[a_idx] < 0 {
        a_val
    } else {
        (a_val * a_r.0) / a_r.1
    };
    let final_d = if args.is_critical && args.defender_stages[d_idx] > 0 {
        d_val
    } else {
        (d_val * d_r.0) / d_r.1
    };

    // 基本ダメージ計算[cite: 18, 26]
    let mut damage = (((22 * power * final_a) / final_d) / 50 + 2) as f64;

    // タイプ一致補正 (STAB)[cite: 20, 26]
    if let Some(atk_types) = master.pokemon_types.get(&args.attacker_id)
        && atk_types.contains(&m.type_id) {
            damage = (damage * 1.5).floor();
        }

    // 乱数・急所補正[cite: 18, 20, 26]
    damage = (damage * args.rng_roll).floor();
    if args.is_critical {
        damage = (damage * 1.5).floor();
    }

    // タイプ相性計算[cite: 18, 20, 26]
    if let Some(target_types) = master.pokemon_types.get(&args.defender_id) {
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
