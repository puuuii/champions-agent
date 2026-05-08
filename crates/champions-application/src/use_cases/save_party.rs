use crate::errors::PartyRepositoryError;
use crate::ports::PartyRepository;
use champions_domain::party::SavedParty;

pub struct SavePartyCommand {
    pub party: SavedParty,
}

#[derive(Debug, Clone)]
pub struct PartyValidationWarning {
    pub pokemon_index: usize,
    pub message: String,
}

pub struct SavePartyResult {
    pub saved_count: usize,
    pub warnings: Vec<PartyValidationWarning>,
}

pub struct SavePartyUseCase<'a> {
    party_repo: &'a dyn PartyRepository,
}

impl<'a> SavePartyUseCase<'a> {
    pub fn new(party_repo: &'a dyn PartyRepository) -> Self {
        Self { party_repo }
    }

    pub fn execute(
        &self,
        command: SavePartyCommand,
    ) -> Result<SavePartyResult, PartyRepositoryError> {
        let warnings = self.validate(&command.party);
        self.party_repo.save_my_party(&command.party)?;
        Ok(SavePartyResult {
            saved_count: command.party.pokemons.len(),
            warnings,
        })
    }

    fn validate(&self, party: &SavedParty) -> Vec<PartyValidationWarning> {
        let mut warnings = Vec::new();
        for (i, pokemon) in party.pokemons.iter().enumerate() {
            if pokemon.species_name.is_empty() {
                warnings.push(PartyValidationWarning {
                    pokemon_index: i,
                    message: "species name is empty".to_string(),
                });
            }
        }
        warnings
    }
}
