use thiserror::Error;

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("catalog data not found: {0}")]
    NotFound(String),
    #[error("catalog load failed: {0}")]
    LoadFailed(String),
}

#[derive(Debug, Error)]
pub enum PartyRepositoryError {
    #[error("party load failed: {0}")]
    LoadFailed(String),
    #[error("party save failed: {0}")]
    SaveFailed(String),
}

#[derive(Debug, Error)]
pub enum UsageError {
    #[error("usage load failed: {0}")]
    LoadFailed(String),
    #[error("usage save failed: {0}")]
    SaveFailed(String),
    #[error("usage not found: {0}")]
    NotFound(String),
}

#[derive(Debug, Error)]
pub enum UsageFetchError {
    #[error("usage fetch failed: {0}")]
    FetchFailed(String),
    #[error("usage parse failed: {0}")]
    ParseFailed(String),
}
