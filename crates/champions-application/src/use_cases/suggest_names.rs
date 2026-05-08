use crate::errors::CatalogError;
use crate::ports::CatalogRepository;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuggestKind {
    Species,
    Move,
    Item,
    Nature,
    Ability,
}

pub struct SuggestNamesQuery {
    pub kind: SuggestKind,
    pub query: String,
    pub limit: usize,
}

pub struct SuggestNamesResult {
    pub suggestions: Vec<String>,
}

pub struct SuggestNamesUseCase<'a> {
    catalog_repo: &'a dyn CatalogRepository,
}

impl<'a> SuggestNamesUseCase<'a> {
    pub fn new(catalog_repo: &'a dyn CatalogRepository) -> Self {
        Self { catalog_repo }
    }

    pub fn execute(&self, query: SuggestNamesQuery) -> Result<SuggestNamesResult, CatalogError> {
        let suggestions = match query.kind {
            SuggestKind::Species => self
                .catalog_repo
                .suggest_species(&query.query, query.limit)?,
            SuggestKind::Move => self.catalog_repo.suggest_moves(&query.query, query.limit)?,
            SuggestKind::Item => self.catalog_repo.suggest_items(&query.query, query.limit)?,
            SuggestKind::Nature => self
                .catalog_repo
                .suggest_natures(&query.query, query.limit)?,
            SuggestKind::Ability => self
                .catalog_repo
                .suggest_abilities(&query.query, query.limit)?,
        };
        Ok(SuggestNamesResult { suggestions })
    }
}
