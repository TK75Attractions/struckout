use std::{fs::File, path::PathBuf};

use chrono::{DateTime, Duration, Local, Utc};
use serde::{Deserialize, Serialize};

use crate::tracking::{AssignedTrackResult, TrackId};

const JSON_LOGGER_FLASH_DURATION: Duration = Duration::minutes(10);

/// Logs tracking events.
pub trait EventLogger {
    fn push_events(&mut self, events: TrackingEventsDto);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingEventsDto {
    pub timestamp: DateTime<Utc>,
    pub events: Vec<TrackingEventBodyDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrackingEventBodyDto {
    NewTrack,
    UpdateTrack(AssignedTrackResult),
    DropTrack(TrackId),
}

/// Logs tracking events to local json files.
pub struct JsonEventLogger {
    dir: PathBuf,
    buf: Vec<TrackingEventsDto>,
    time_to_next_flash: Duration,
    file: Option<File>,
}

impl JsonEventLogger {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            buf: Vec::new(),
            time_to_next_flash: JSON_LOGGER_FLASH_DURATION,
            file: None,
        }
    }
}

impl EventLogger for JsonEventLogger {
    fn push_events(&mut self, events: TrackingEventsDto) {
        if self.file.is_none() {
            let fname = format!("{}", Local::now().format("%Y%m%d_%H%M"));
            let fpath = self.dir.join(fname);
            let file = File::create(fpath).expect("failed to create file");

            self.file = Some(file)
        }

        serde_json::to_writer(self.file.as_mut().unwrap(), &events)
            .expect("failed to write events to file");
    }
}

/// Logs tracking events to sentry.
pub struct SentryEventLogger {}

impl SentryEventLogger {
    pub fn new() -> Self {
        Self {}
    }
}

impl EventLogger for SentryEventLogger {
    fn push_events(&mut self, _events: TrackingEventsDto) {
        todo!()
    }
}

/// Dummy [`EventLogger`].
pub struct EmptyEventLogger;

impl EventLogger for EmptyEventLogger {
    fn push_events(&mut self, _events: TrackingEventsDto) {
        todo!()
    }
}
