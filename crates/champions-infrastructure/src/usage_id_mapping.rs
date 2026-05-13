pub fn resolve_master_pokemon_id(gamewith_id: &str, name: &str) -> Option<u32> {
    match gamewith_id.trim() {
        // Regional forms
        "026_1" => Some(10100), // raichu-alola
        "038_1" => Some(10104), // ninetales-alola
        "059_1" => Some(10230), // arcanine-hisui
        "080_2" => Some(10165), // slowbro-galar
        "128_1" => Some(10250), // tauros-paldea-combat-breed
        "128_2" => Some(10252), // tauros-paldea-aqua-breed
        "128_3" => Some(10251), // tauros-paldea-blaze-breed
        "157_1" => Some(10233), // typhlosion-hisui
        "199_1" => Some(10172), // slowking-galar
        "503_1" => Some(10236), // samurott-hisui
        "571_1" => Some(10239), // zoroark-hisui
        "618_1" => Some(10180), // stunfisk-galar
        "713_1" => Some(10243), // avalugg-hisui
        "724_1" => Some(10244), // decidueye-hisui

        // Appliance forms
        "479_1" => Some(10008), // rotom-heat
        "479_2" => Some(10009), // rotom-wash
        "479_3" => Some(10010), // rotom-frost
        "479_4" => Some(10011), // rotom-fan (スピンロトム)
        "479_5" => Some(10012), // rotom-mow (カットロトム)

        // Sex / special forms
        "670_2" => Some(10061), // floette-eternal
        "678_1" => Some(10025), // meowstic-female
        "706_1" => Some(10242), // goodra-hisui
        "745_1" => Some(10126), // lycanroc-midnight
        "745_2" => Some(10152), // lycanroc-dusk
        "902_1" => Some(10248), // basculegion-female

        // Size forms. GameWith uses different suffix numbering here than master defaults.
        "711" if name.trim() == "パンプジン(こだま)" => Some(10030),   // gourgeist-small
        "711_1" => Some(711),   // gourgeist-average
        "711_2" => Some(10031), // gourgeist-large
        "711_3" => Some(10032), // gourgeist-super

        _ => gamewith_id.trim().parse::<u32>().ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_master_pokemon_id;

    #[test]
    fn resolves_plain_numeric_ids_to_master_defaults() {
        assert_eq!(resolve_master_pokemon_id("445", "ガブリアス"), Some(445));
        assert_eq!(resolve_master_pokemon_id("678", "ニャオニクス(オス)"), Some(678));
        assert_eq!(resolve_master_pokemon_id("902", "イダイトウ(オス)"), Some(902));
    }

    #[test]
    fn resolves_all_current_form_suffix_overrides() {
        let cases = [
            ("026_1", "アローラライチュウ", 10100),
            ("038_1", "アローラキュウコン", 10104),
            ("059_1", "ヒスイウインディ", 10230),
            ("080_2", "ガラルヤドラン", 10165),
            ("128_1", "パルデアケンタロス(闘)", 10250),
            ("128_2", "パルデアケンタロス(水)", 10252),
            ("128_3", "パルデアケンタロス(炎)", 10251),
            ("157_1", "ヒスイバクフーン", 10233),
            ("199_1", "ガラルヤドキング", 10172),
            ("479_1", "ヒートロトム", 10008),
            ("479_2", "ウォッシュロトム", 10009),
            ("479_3", "フロストロトム", 10010),
            ("479_4", "スピンロトム", 10011),
            ("479_5", "カットロトム", 10012),
            ("503_1", "ヒスイダイケンキ", 10236),
            ("571_1", "ヒスイゾロアーク", 10239),
            ("618_1", "ガラルマッギョ", 10180),
            ("670_2", "フラエッテ(永遠)", 10061),
            ("678_1", "ニャオニクス(メス)", 10025),
            ("706_1", "ヒスイヌメルゴン", 10242),
            ("711_1", "パンプジン(ちゅうだま)", 711),
            ("711_2", "パンプジン(おおだま)", 10031),
            ("711_3", "パンプジン(ギガだま)", 10032),
            ("713_1", "ヒスイクレベース", 10243),
            ("724_1", "ヒスイジュナイパー", 10244),
            ("745_1", "ルガルガン(夜)", 10126),
            ("745_2", "ルガルガン(黄昏)", 10152),
            ("902_1", "イダイトウ(メス)", 10248),
        ];

        for (gamewith_id, name, expected) in cases {
            assert_eq!(
                resolve_master_pokemon_id(gamewith_id, name),
                Some(expected),
                "failed to map {gamewith_id} / {name}",
            );
        }
    }

    #[test]
    fn resolves_gourgeist_small_unsuffixed_exception() {
        assert_eq!(
            resolve_master_pokemon_id("711", "パンプジン(こだま)"),
            Some(10030)
        );
        assert_eq!(resolve_master_pokemon_id("711", "パンプジン"), Some(711));
    }
}
