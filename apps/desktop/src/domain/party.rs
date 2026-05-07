use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SavedPokemon {
    pub species: String,
    pub item: String,
    pub h: String,
    pub a: String,
    pub b: String,
    pub c: String,
    pub d: String,
    pub s: String,
    pub nature: String,
    pub ability: String,
    pub moves: [String; 4],
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SavedParty {
    pub pokemons: Vec<SavedPokemon>,
}

pub struct Pokemon {
    pub name: String,
}

pub trait PartyRepository {
    fn find_best_match(&self, image_data: &[u8]) -> Option<(String, f32)>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockRepo;
    impl PartyRepository for MockRepo {
        fn find_best_match(&self, _data: &[u8]) -> Option<(String, f32)> {
            Some(("Pikachu".to_string(), 1.0))
        }
    }

    #[test]
    fn test_mock_repository() {
        let repo = MockRepo;
        let result = repo.find_best_match(&[]);
        assert!(result.is_some());
        assert_eq!(result.unwrap().0, "Pikachu");
    }
}
