use std::fmt;

use struckout_proto::types::MachineId;
use tracing::{Event, Subscriber};
use tracing_subscriber::{
    fmt::{FmtContext, FormatEvent, FormatFields, format::Writer},
    registry::LookupSpan,
};

/// Adds the machine to every event, including events from spawned worker threads.
pub(crate) struct MachineEventFormat(pub MachineId);

impl<S, N> FormatEvent<S, N> for MachineEventFormat
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        context: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        write!(writer, "machine_id={} ", self.0)?;
        tracing_subscriber::fmt::format().format_event(context, writer, event)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io,
        sync::{Arc, Mutex},
    };

    use super::*;

    #[derive(Clone, Default)]
    struct Output(Arc<Mutex<Vec<u8>>>);

    impl io::Write for Output {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0
                .lock()
                .expect("log capture lock")
                .extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn machine_id_is_present_without_an_entered_span_on_worker_threads() {
        let output = Output::default();
        let capture = output.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .event_format(MachineEventFormat(MachineId::new(2)))
            .with_writer(move || capture.clone())
            .finish();
        let dispatch = tracing::Dispatch::new(subscriber);
        std::thread::spawn(move || {
            tracing::dispatcher::with_default(&dispatch, || tracing::info!("worker event"));
        })
        .join()
        .expect("worker should complete");
        let bytes = output.0.lock().expect("log capture lock");
        let log = std::str::from_utf8(&bytes).expect("UTF-8 log");
        assert!(log.contains("machine_id=2 "), "{log}");
        assert!(log.contains("worker event"), "{log}");
    }
}
