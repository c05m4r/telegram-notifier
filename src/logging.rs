/// Inicializa el sistema de logs con timestamps RFC3339 y filtro por nivel.
///
/// Nivel configurable con la variable `RUST_LOG` (ej: `RUST_LOG=debug`,
/// `RUST_LOG=telegram_notifier=debug,info`). Default: `info`.
pub fn init() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_timer(tracing_subscriber::fmt::time::ChronoLocal::rfc_3339())
        .init();
}