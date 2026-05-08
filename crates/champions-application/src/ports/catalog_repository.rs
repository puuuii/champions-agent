use crate::errors::CatalogError;
use champions_domain::catalog::BattleMasterData;

pub trait CatalogRepository: Send + Sync {
    fn suggest_species(&self, query: &str, limit: usize) -> Result<Vec<String>, CatalogError>;
    fn suggest_moves(&self, query: &str, limit: usize) -> Result<Vec<String>, CatalogError>;
    fn suggest_items(&self, query: &str, limit: usize) -> Result<Vec<String>, CatalogError>;
    fn suggest_natures(&self, query: &str, limit: usize) -> Result<Vec<String>, CatalogError>;
    fn suggest_abilities(&self, query: &str, limit: usize) -> Result<Vec<String>, CatalogError>;
    fn load_battle_master_data(&self) -> Result<BattleMasterData, CatalogError>;
}
