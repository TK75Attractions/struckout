use std::{
    fs::{self, File},
    io::Write,
    path::PathBuf,
};

use chrono::{DateTime, Duration, Local, Utc};
use serde::{Deserialize, Serialize};
use struckout_proto::CameraLocation;
use tracing::trace;

use crate::{
    detection_input::PairedFrames,
    tracking::{AssignedTrackResult, TrackId},
};

const JSON_LOGGER_FLASH_DURATION: Duration = Duration::minutes(10);

/// Logs tracking events.
pub trait EventLogger {
    fn push_events(&mut self, events: TrackingEventsDto);

    fn push_pair(&mut self, pair: &PairedFrames);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingEventsDto {
    pub timestamp: DateTime<Utc>,
    pub events: Vec<TrackingEventBodyDto>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NewTrackDiagnosticsDto {
    pub camera_location_a: CameraLocation,
    pub camera_location_b: CameraLocation,
    pub unmatched_detections_a: usize,
    pub unmatched_detections_b: usize,
    pub candidate_pairs: usize,
    pub parallel_ray_rejections: usize,
    pub behind_camera_rejections: usize,
    pub ray_distance_rejections: usize,
    pub within_gate_candidates: usize,
    pub unselected_within_gate_candidates: usize,
    pub min_ray_distance: Option<f64>,
    pub accepted_tracks: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrackingEventBodyDto {
    NewTrack,
    NewTrackDiagnostics(NewTrackDiagnosticsDto),
    UpdateTrack(AssignedTrackResult),
    DropTrack(TrackId),
}

/// Logs tracking events to local json files.
pub struct JsonEventLogger {
    dir: PathBuf,
    buf: Vec<TrackingEventsDto>,
    events_file: File,
    pair_file: File,
}

impl JsonEventLogger {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        let events_file = {
            let fname = format!("events_{}", Local::now().format("%Y%m%d_%H%M"));
            let fpath = dir.join(fname);
            fs::create_dir_all(fpath.parent().unwrap()).expect("faild to create parent dirs");
            File::create(fpath).expect("failed to create file")
        };
        let pair_file = {
            let fname = format!("pair_{}", Local::now().format("%Y%m%d_%H%M"));
            let fpath = dir.join(fname);
            File::create(fpath).expect("failed to create pair file")
        };

        Self {
            dir,
            buf: Vec::new(),
            events_file,
            pair_file,
        }
    }
}

impl EventLogger for JsonEventLogger {
    fn push_events(&mut self, events: TrackingEventsDto) {
        serde_json::to_writer_pretty(&mut self.events_file, &events)
            .expect("failed to write events to file");
        self.events_file
            .flush()
            .expect("failed to flush events to file");
    }

    fn push_pair(&mut self, pair: &PairedFrames) {
        serde_json::to_writer_pretty(&mut self.pair_file, &pair)
            .expect("failed to write events to file");
        self.pair_file
            .flush()
            .expect("failed to flush pairs to file");
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

    fn push_pair(&mut self, _pair: &PairedFrames) {
        todo!()
    }
}

/// [`EventLogger`] to stdout.
pub struct FmtEventLogger;

impl EventLogger for FmtEventLogger {
    fn push_events(&mut self, events: TrackingEventsDto) {
        trace!(?events, "new event");
    }

    fn push_pair(&mut self, _pair: &PairedFrames) {
        todo!()
    }
}
