use anyhow::Result;
use champions_infrastructure::config::AppPaths;
use std::sync::OnceLock;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    EnvFilter,
    fmt::{self, format::FmtSpan},
    prelude::*,
};

static FILE_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

const DEFAULT_LOG_FILTER: &str =
    "info,champions_runtime=debug,champions_infrastructure=info,champions_agent_desktop=info";

pub fn init(app_paths: &AppPaths) -> Result<()> {
    let active_filter = active_filter();
    let log_dir = app_paths.debug_dir.join("logs");

    match std::fs::create_dir_all(&log_dir) {
        Ok(()) => {
            let console_layer = fmt::layer()
                .compact()
                .with_target(true)
                .with_thread_ids(true)
                .with_thread_names(true)
                .with_file(true)
                .with_line_number(true);
            let file_appender = tracing_appender::rolling::daily(&log_dir, "champions-agent.log");
            let (file_writer, guard) = tracing_appender::non_blocking(file_appender);
            let file_layer = fmt::layer()
                .json()
                .with_ansi(false)
                .with_current_span(true)
                .with_span_list(true)
                .with_thread_ids(true)
                .with_thread_names(true)
                .with_target(true)
                .with_file(true)
                .with_line_number(true)
                .with_writer(file_writer)
                .with_span_events(FmtSpan::CLOSE);

            tracing_subscriber::registry()
                .with(build_env_filter())
                .with(console_layer)
                .with(file_layer)
                .try_init()?;

            let _ = FILE_GUARD.set(guard);
            tracing::info!(
                version = env!("CARGO_PKG_VERSION"),
                filter = %active_filter,
                log_dir = %log_dir.display(),
                "observability initialized",
            );
        }
        Err(error) => {
            let console_layer = fmt::layer()
                .compact()
                .with_target(true)
                .with_thread_ids(true)
                .with_thread_names(true)
                .with_file(true)
                .with_line_number(true);
            tracing_subscriber::registry()
                .with(build_env_filter())
                .with(console_layer)
                .try_init()?;

            tracing::warn!(
                %error,
                "file logging is disabled; continuing with console logging only",
            );
            tracing::info!(
                version = env!("CARGO_PKG_VERSION"),
                filter = %active_filter,
                "observability initialized",
            );
        }
    }

    Ok(())
}

fn build_env_filter() -> EnvFilter {
    EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(DEFAULT_LOG_FILTER))
        .expect("default tracing filter must be valid")
}

fn active_filter() -> String {
    std::env::var("RUST_LOG").unwrap_or_else(|_| DEFAULT_LOG_FILTER.to_string())
}
