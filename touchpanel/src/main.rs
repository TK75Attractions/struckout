use touchpanel::{run_main, version_option};
use tracing::Level;
use tracing_subscriber::{Layer, filter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

#[cfg(feature = "sentry")]
const SENTRY_DSN: &str = "https://2b9af00fb25f2faa692253f8ed95c43e@o4512229597315072.ingest.us.sentry.io/4512229769871360";

fn main() -> std::process::ExitCode {
    let fmt_layer = fmt::layer().with_filter(filter::filter_fn(|meta| {
        meta.module_path()
            .map(|p| {
                if p.starts_with("touchpanel") {
                    true
                } else {
                    *meta.level() <= Level::DEBUG
                }
            })
            .unwrap_or(*meta.level() >= Level::DEBUG)
    }));

    #[cfg(feature = "sentry")]
    let _sentry_guard = sentry::init(
        sentry::ClientOptions::new()
            .dsn(SENTRY_DSN)
            .maybe_release(version_option()),
    );

    let reg = tracing_subscriber::registry().with(fmt_layer);
    #[cfg(feature = "sentry")]
    let reg = reg.with(sentry::integrations::tracing::layer());
    reg.init();

    #[cfg(feature = "sentry")]
    tracing::info!("initialized tracing with sentry layer");

    run_main();
    std::process::ExitCode::SUCCESS
}
