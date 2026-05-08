use std::collections::HashMap;

use crate::errors::UsageError;
use crate::ports::{
    PartyIdentifier, PartyIdentifierError, PartyImageSet, RecognitionConfig, UsageRepository,
};
use champions_domain::recognition::{RecognitionConflict, RecognizedParty, SelectionSlot};
use champions_domain::usage::PokemonUsageSummary;

pub struct IdentifyOpponentPartyCommand {
    pub party_images: PartyImageSet,
    pub config: RecognitionConfig,
}

#[derive(Debug, Clone)]
pub struct OpponentPartyIdentificationResult {
    pub recognized_party: RecognizedParty,
    pub usage_summaries: Vec<PokemonUsageSummary>,
    pub conflicts: Vec<RecognitionConflict>,
}

#[derive(Debug, thiserror::Error)]
pub enum IdentifyOpponentPartyError {
    #[error("party identification failed: {0}")]
    IdentifierError(#[from] PartyIdentifierError),
    #[error("usage lookup failed: {0}")]
    UsageError(#[from] UsageError),
}

pub struct IdentifyOpponentPartyUseCase<'a> {
    party_identifier: &'a dyn PartyIdentifier,
    usage_repo: &'a dyn UsageRepository,
}

impl<'a> IdentifyOpponentPartyUseCase<'a> {
    pub fn new(
        party_identifier: &'a dyn PartyIdentifier,
        usage_repo: &'a dyn UsageRepository,
    ) -> Self {
        Self {
            party_identifier,
            usage_repo,
        }
    }

    pub fn execute(
        &self,
        command: IdentifyOpponentPartyCommand,
    ) -> Result<OpponentPartyIdentificationResult, IdentifyOpponentPartyError> {
        let recognized_party = self
            .party_identifier
            .identify_opponent_party(&command.party_images, &command.config)?;

        let names: Vec<String> = recognized_party
            .pokemons
            .iter()
            .filter_map(|p| p.display_name.clone())
            .collect();

        let usage_summaries = if names.is_empty() {
            Vec::new()
        } else {
            self.usage_repo.find_many_by_names(&names)?
        };

        let conflicts = detect_conflicts(&recognized_party);

        Ok(OpponentPartyIdentificationResult {
            recognized_party,
            usage_summaries,
            conflicts,
        })
    }
}

fn detect_conflicts(party: &RecognizedParty) -> Vec<RecognitionConflict> {
    let mut name_slots: HashMap<String, Vec<SelectionSlot>> = HashMap::new();

    for pokemon in &party.pokemons {
        if let Some(name) = &pokemon.display_name {
            name_slots
                .entry(name.clone())
                .or_default()
                .push(pokemon.slot);
        }
    }

    name_slots
        .into_iter()
        .filter(|(_, slots)| slots.len() > 1)
        .map(|(species_name, slots)| RecognitionConflict {
            species_name,
            slots,
        })
        .collect()
}
