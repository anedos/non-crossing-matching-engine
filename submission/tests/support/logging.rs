use std::sync::Once;

/// Enable test diagnostics through RUST_LOG, remaining silent by default.
pub(crate) fn init_test_logging() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = env_logger::Builder::new()
            .filter_level(log::LevelFilter::Off)
            .parse_env(env_logger::Env::default().default_filter_or("off"))
            .target(env_logger::Target::Stderr)
            .write_style(env_logger::WriteStyle::Never)
            .format_timestamp(None)
            .is_test(true)
            .try_init();
    });
}
