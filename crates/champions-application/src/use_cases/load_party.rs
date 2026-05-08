use crate::errors::PartyRepositoryError;
use crate::ports::PartyRepository;
use champions_domain::party::SavedParty;

pub struct LoadPartyResult {
    pub party: SavedParty,
}

pub struct LoadPartyUseCase<'a> {
    party_repo: &'a dyn PartyRepository,
}

impl<'a> LoadPartyUseCase<'a> {
    pub fn new(party_repo: &'a dyn PartyRepository) -> Self {
        Self { party_repo }
    }

    pub fn execute(&self) -> Result<LoadPartyResult, PartyRepositoryError> {
        let party = self.party_repo.load_my_party()?;
        Ok(LoadPartyResult { party })
    }
}
