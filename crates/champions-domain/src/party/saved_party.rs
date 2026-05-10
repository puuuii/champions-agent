use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SavedParty {
    #[serde(default)]
    pub pokemons: Vec<PokemonBuild>,
    #[serde(default)]
    pub saved_pokemons: Vec<PokemonBuild>,
}

impl SavedParty {
    pub fn remember_pokemon(&mut self, pokemon: PokemonBuild) {
        if !pokemon.is_blank() {
            self.saved_pokemons.push(pokemon);
        }
    }

    pub fn remember_pokemons<I>(&mut self, pokemons: I)
    where
        I: IntoIterator<Item = PokemonBuild>,
    {
        for pokemon in pokemons {
            self.remember_pokemon(pokemon);
        }
    }

    pub fn has_saved_pokemon_equivalent(&self, pokemon: &PokemonBuild) -> bool {
        self.saved_pokemons
            .iter()
            .any(|saved| saved.is_equivalent(pokemon))
    }

    pub fn has_saved_pokemon_equivalent_except(
        &self,
        pokemon: &PokemonBuild,
        excluded_index: usize,
    ) -> bool {
        self.saved_pokemons
            .iter()
            .enumerate()
            .any(|(index, saved)| index != excluded_index && saved.is_equivalent(pokemon))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PokemonBuild {
    pub species_name: String,
    pub item_name: Option<String>,
    pub ability_name: Option<String>,
    pub nature_name: Option<String>,
    pub effort_values: EffortValueSpread,
    pub moves: MoveSet,
}

impl PokemonBuild {
    pub fn is_blank(&self) -> bool {
        self.species_name.trim().is_empty()
            && self
                .item_name
                .as_deref()
                .is_none_or(|item| item.trim().is_empty())
            && self
                .ability_name
                .as_deref()
                .is_none_or(|ability| ability.trim().is_empty())
            && self
                .nature_name
                .as_deref()
                .is_none_or(|nature| nature.trim().is_empty())
            && self.effort_values.is_zero()
            && self
                .moves
                .moves
                .iter()
                .all(|move_name| move_name.trim().is_empty())
    }

    pub fn is_equivalent(&self, other: &Self) -> bool {
        self.species_name.trim() == other.species_name.trim()
            && normalized_optional_str(self.item_name.as_deref())
                == normalized_optional_str(other.item_name.as_deref())
            && normalized_optional_str(self.ability_name.as_deref())
                == normalized_optional_str(other.ability_name.as_deref())
            && normalized_optional_str(self.nature_name.as_deref())
                == normalized_optional_str(other.nature_name.as_deref())
            && self.effort_values == other.effort_values
            && self
                .moves
                .moves
                .iter()
                .zip(other.moves.moves.iter())
                .all(|(left, right)| left.trim() == right.trim())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffortValueSpread {
    pub h: u32,
    pub a: u32,
    pub b: u32,
    pub c: u32,
    pub d: u32,
    pub s: u32,
}

impl EffortValueSpread {
    pub fn is_zero(&self) -> bool {
        self.h == 0 && self.a == 0 && self.b == 0 && self.c == 0 && self.d == 0 && self.s == 0
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MoveSet {
    pub moves: [String; 4],
}

fn normalized_optional_str(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{EffortValueSpread, MoveSet, PokemonBuild, SavedParty};

    #[test]
    fn equivalent_builds_ignore_surrounding_whitespace_and_empty_options() {
        let left = PokemonBuild {
            species_name: " ガブリアス ".to_string(),
            item_name: Some(" きあいのタスキ ".to_string()),
            ability_name: Some(" さめはだ ".to_string()),
            nature_name: Some(" ".to_string()),
            effort_values: EffortValueSpread {
                h: 0,
                a: 252,
                b: 0,
                c: 0,
                d: 4,
                s: 252,
            },
            moves: MoveSet {
                moves: [
                    " じしん ".to_string(),
                    " げきりん ".to_string(),
                    "".to_string(),
                    " つるぎのまい ".to_string(),
                ],
            },
        };
        let right = PokemonBuild {
            species_name: "ガブリアス".to_string(),
            item_name: Some("きあいのタスキ".to_string()),
            ability_name: Some("さめはだ".to_string()),
            nature_name: None,
            effort_values: EffortValueSpread {
                h: 0,
                a: 252,
                b: 0,
                c: 0,
                d: 4,
                s: 252,
            },
            moves: MoveSet {
                moves: [
                    "じしん".to_string(),
                    "げきりん".to_string(),
                    " ".to_string(),
                    "つるぎのまい".to_string(),
                ],
            },
        };

        assert!(left.is_equivalent(&right));
    }

    #[test]
    fn saved_party_detects_equivalent_saved_pokemon_except_target() {
        let duplicate = PokemonBuild {
            species_name: "カイリュー".to_string(),
            item_name: Some("こだわりハチマキ".to_string()),
            ability_name: Some("マルチスケイル".to_string()),
            nature_name: None,
            effort_values: EffortValueSpread {
                h: 4,
                a: 252,
                b: 0,
                c: 0,
                d: 0,
                s: 252,
            },
            moves: MoveSet {
                moves: [
                    "しんそく".to_string(),
                    "じしん".to_string(),
                    "げきりん".to_string(),
                    "ほのおのパンチ".to_string(),
                ],
            },
        };
        let party = SavedParty {
            pokemons: Vec::new(),
            saved_pokemons: vec![duplicate.clone(), PokemonBuild::default()],
        };

        assert!(party.has_saved_pokemon_equivalent(&duplicate));
        assert!(!party.has_saved_pokemon_equivalent_except(&duplicate, 0));
        assert!(party.has_saved_pokemon_equivalent_except(&duplicate, 1));
    }
}
