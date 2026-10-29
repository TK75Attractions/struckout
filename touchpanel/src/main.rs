use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

use touchpanel::run_main;
use tracing::Level;
use tracing_subscriber::{
    Layer,
    filter::{self, LevelFilter},
    fmt,
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

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
            .maybe_release(touchpanel::version_option()),
    );

    let reg = tracing_subscriber::registry().with(fmt_layer);
    #[cfg(feature = "sentry")]
    let reg = reg
        .with(sentry::integrations::tracing::SpanLayer::new().with_filter(LevelFilter::TRACE))
        .with(sentry::integrations::tracing::LogLayer::new().with_filter(LevelFilter::TRACE));
    reg.init();

    #[cfg(feature = "sentry")]
    tracing::info!("initialized tracing with sentry layer");

    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let signal_shutdown = Arc::clone(&shutdown_requested);
    let (done_tx, done_rx) = tokio::sync::oneshot::channel();
    let signal_thread = thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("signal runtime should be created");
        runtime.block_on(async move {
            tokio::select! {
                _ = shutdown_signal() => signal_shutdown.store(true, Ordering::Release),
                _ = done_rx => {}
            }
        });
    });

    let result = run_main(shutdown_requested);
    let _ = done_tx.send(());
    if signal_thread.join().is_err() {
        eprintln!("終了シグナルの待機スレッドが異常終了しました");
        return std::process::ExitCode::FAILURE;
    }

    if let Err(error) = result {
        eprintln!("touchpanel を起動・実行できません: {error}");
        return std::process::ExitCode::FAILURE;
    }

    std::process::ExitCode::SUCCESS
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::SignalKind;

        let mut terminate = tokio::signal::unix::signal(SignalKind::terminate())
            .expect("SIGTERM handler should be installed");
        tokio::select! {
            result = tokio::signal::ctrl_c() => result.expect("Ctrl-C handler should be installed"),
            _ = terminate.recv() => {}
        }
    }

    #[cfg(not(unix))]
    tokio::signal::ctrl_c()
        .await
        .expect("Ctrl-C handler should be installed");
}
