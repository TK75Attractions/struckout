use touchpanel::run_main;
use tracing::Level;
use tracing_subscriber::{Layer, filter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

fn main() {
    let layer = fmt::layer().with_filter(filter::filter_fn(|meta| {
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
    tracing_subscriber::registry().with(layer).init();

    run_main();
}
