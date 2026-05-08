use crate::errors::{UsageError, UsageFetchError};
use crate::ports::{UsageFetcher, UsageRepository, UsageSource};

pub struct RefreshUsageDataCommand {
    pub source: UsageSource,
}

pub struct RefreshUsageDataResult {
    pub count: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum RefreshUsageDataError {
    #[error(transparent)]
    Fetch(#[from] UsageFetchError),
    #[error(transparent)]
    Repository(#[from] UsageError),
}

pub struct RefreshUsageDataUseCase<'a> {
    fetcher: &'a dyn UsageFetcher,
    usage_repo: &'a dyn UsageRepository,
}

impl<'a> RefreshUsageDataUseCase<'a> {
    pub fn new(fetcher: &'a dyn UsageFetcher, usage_repo: &'a dyn UsageRepository) -> Self {
        Self {
            fetcher,
            usage_repo,
        }
    }

    pub fn execute(
        &self,
        command: RefreshUsageDataCommand,
    ) -> Result<RefreshUsageDataResult, RefreshUsageDataError> {
        let data = self.fetcher.fetch_usage(command.source)?;
        let count = data.len();
        self.usage_repo.replace_all(data)?;
        Ok(RefreshUsageDataResult { count })
    }
}
