use tracing_subscriber::EnvFilter;

const DEFAULT_FILTER: &str = "info,shrimply=debug";

/// Installs the process-wide diagnostics subscriber.
///
/// Application code emits `tracing` spans and events. Logs emitted by dependencies through the
/// `log` facade are forwarded by `tracing-subscriber` into the same output.
pub fn init() {
    let filter = EnvFilter::new(std::env::var("RUST_LOG").map_or_else(
        |_| DEFAULT_FILTER.to_string(),
        |directives| format!("{DEFAULT_FILTER},{directives}"),
    ));
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(filter)
        .with_thread_ids(true)
        .with_thread_names(true)
        .try_init()
        .expect("diagnostics subscriber should only be installed once");
}
