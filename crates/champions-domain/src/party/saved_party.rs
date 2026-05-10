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
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
