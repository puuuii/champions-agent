use crate::errors::PartyRepositoryError;
use champions_domain::party::SavedParty;

pub trait PartyRepository: Send + Sync {
    fn load_my_party(&self) -> Result<SavedParty, PartyRepositoryError>;
    fn save_my_party(&self, party: &SavedParty) -> Result<(), PartyRepositoryError>;
}
