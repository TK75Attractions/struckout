use touchpanel::run_main;
use tracing::Level;
use tracing_subscriber::{Layer, filter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

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
    let _sentry_guard =sentry::init(sentry::ClientOptions::new()
        .dsn("https://2b9af00fb25f2faa692253f8ed95c43e@o4512229597315072.ingest.us.sentry.io/4512229769871360")
        .maybe_release(sentry::release_name!()));
    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(sentry::integrations::tracing::layer())
        .init();

    run_main();
    std::process::ExitCode::SUCCESS
}
