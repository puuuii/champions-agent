use crate::errors::CatalogError;
use crate::ports::CatalogRepository;
use champions_domain::battle::{DamageCalcError, DamageInput, calculate_damage};

pub struct CalculateDamageCommand {
    pub input: DamageInput,
}

pub struct CalculateDamageResult {
    pub damage: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum CalculateDamageError {
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Calculation(#[from] DamageCalcError),
}

pub struct CalculateDamageUseCase<'a> {
    catalog_repo: &'a dyn CatalogRepository,
}

impl<'a> CalculateDamageUseCase<'a> {
    pub fn new(catalog_repo: &'a dyn CatalogRepository) -> Self {
        Self { catalog_repo }
    }

    pub fn execute(
        &self,
        command: CalculateDamageCommand,
    ) -> Result<CalculateDamageResult, CalculateDamageError> {
        let master = self.catalog_repo.load_battle_master_data()?;
        let damage = calculate_damage(&master, &command.input)?;
        Ok(CalculateDamageResult { damage })
    }
}
