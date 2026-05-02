pub mod calc;
pub mod loader;
pub mod models;

pub use calc::*;
pub use models::*;

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::path::PathBuf;

    // ヘルパー：マスタデータのロード
    fn get_master_data() -> MasterData {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("master_data");
        MasterData::load_from_dir(path).expect("マスタデータの読み込みに失敗しました")
    }

    // デフォルトの引数生成
    fn default_args(attacker: u32, defender: u32, move_id: u32) -> DamageArgs {
        DamageArgs {
            attacker_id: attacker,
            defender_id: defender,
            move_id: move_id,
            attacker_ap: [0; 6],
            defender_ap: [0; 6],
            attacker_nature_id: 1,
            defender_nature_id: 1,
            attacker_stages: [0; 8],
            defender_stages: [0; 8],
            attacker_status_id: None,
            is_critical: false,
            rng_roll: 1.0, // 最大乱数
        }
    }

    /// 1. ダメージ計算の基本ロジック・タイプ相性・乱数の網羅テスト
    #[rstest]
    #[case::immunity(445, 6, 89, 0, 1.0, false, 0)]
    #[case::zero_power(445, 6, 45, 0, 1.0, false, 0)]
    #[case::min_rng(445, 445, 89, 0, 0.85, false, 82)]
    #[case::critical(445, 445, 89, 0, 1.0, true, 145)]
    fn test_damage_variations(
        #[case] atk_id: u32,
        #[case] def_id: u32,
        #[case] move_id: u32,
        #[case] ap_a: u32,
        #[case] rng: f64,
        #[case] crit: bool,
        #[case] expected: u32,
    ) {
        let master = get_master_data();
        let mut args = default_args(atk_id, def_id, move_id);
        args.attacker_ap = [0, ap_a, 0, 0, 0, 0];
        args.rng_roll = rng;
        args.is_critical = crit;

        let result = calculate_damage(&master, &args).unwrap();
        assert_eq!(result, expected, "Damage calculation mismatch for case");
    }

    /// 2. ランク補正（ステージ）と急所の相互作用テスト
    #[rstest]
    #[case::atk_plus_6(6, 0, 0, 1.0, false, 381)]
    #[case::def_plus_6(0, 6, 0, 1.0, false, 25)]
    #[case::atk_minus_6(-6, 0, 0, 1.0, false, 25)]
    #[case::crit_ignores_my_debuff(-6, 0, 0, 1.0, true, 145)]
    #[case::crit_ignores_opp_buff(0, 6, 0, 1.0, true, 145)]
    fn test_rank_stages(
        #[case] a_stage: i8,
        #[case] d_stage: i8,
        #[case] ap_a: u32,
        #[case] rng: f64,
        #[case] crit: bool,
        #[case] expected: u32,
    ) {
        let master = get_master_data();
        // ガブリアス(445) vs ガブリアス(445), じしん(89)
        let mut args = default_args(445, 445, 89);
        args.attacker_ap = [0, ap_a, 0, 0, 0, 0];
        args.attacker_stages = [0, a_stage, 0, 0, 0, 0, 0, 0];
        args.defender_stages = [0, 0, d_stage, 0, 0, 0, 0, 0];
        args.rng_roll = rng;
        args.is_critical = crit;

        let result = calculate_damage(&master, &args).unwrap();
        assert_eq!(result, expected);
    }

    /// 3. エラー系（存在しないID）のテスト
    #[rstest]
    #[case::invalid_attacker(9999, 6, 89, "Attacker not found")]
    #[case::invalid_defender(445, 9999, 89, "Defender not found")]
    #[case::invalid_move(445, 6, 9999, "Move not found")]
    fn test_error_cases(
        #[case] atk_id: u32,
        #[case] def_id: u32,
        #[case] move_id: u32,
        #[case] expected_msg: &str,
    ) {
        let master = get_master_data();
        let args = default_args(atk_id, def_id, move_id);

        let result = calculate_damage(&master, &args);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), expected_msg);
    }

    /// 4. データ整合性テスト：複数タイプ等のロード確認
    #[test]
    fn test_master_data_integrity() {
        let master = get_master_data();

        // リザードン(6) が 炎(10) と 飛行(3) の2つのタイプを持っているか
        let types = master
            .pokemon_types
            .get(&6)
            .expect("Charizard types not found");
        assert!(types.contains(&10));
        assert!(types.contains(&3));
        assert_eq!(types.len(), 2);

        // ガブリアス(445) の種族値 A が 130 であるか
        let stats = master
            .pokemon_stats
            .get(&445)
            .expect("Garchomp stats not found");
        assert_eq!(stats[1], 130);
    }
}
