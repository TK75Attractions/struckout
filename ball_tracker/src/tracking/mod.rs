use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::{
    CameraLocationStore,
    detection_input::PairedFrames,
    tracking::{
        data_association::associate_objects,
        triangulate::{TriangulationError, triangulate},
    },
    types::{CameraId, CollisionPoint3D, GetLayFromDetection as _, Position3D, RotateToWorld as _},
};

mod data_association;
mod event;
pub use event::*;
mod kalman;
pub use kalman::KalmanTrack;
mod triangulate;

use anyhow::Context;
use chrono::{DateTime, Utc};
use nalgebra::Vector3;
use serde::{Deserialize, Serialize};
use struckout_proto::Detection;
use tokio::sync::mpsc;
use tracing::trace;
use tracktor::assignment::{CostMatrix, hungarian_gated};

const NEW_TRACK_RAY_DISTANCE_GATE: f64 = 10.0;
const MAX_MISSED_UPDATES: usize = 3;

pub struct TrackRunner<T, EL> {
    tracks: HashMap<TrackId, T>,
    missed_updates: HashMap<TrackId, usize>,
    camera_locs: Arc<CameraLocationStore>,
    id_gen: TrackIdGenerator,
    event_logger: EL,
}

/// Tracks an object.
pub trait ObjectTrack {
    fn new(
        id: TrackId,
        initial_position: Vector3<f64>,
        timestamp: DateTime<Utc>,
        camera_loc_provider: Arc<CameraLocationStore>,
    ) -> Self;

    fn id(&self) -> TrackId;

    /// Predict object location and evaluate scores for each detections.
    fn evaluate_scores<'a>(
        &mut self,
        camera_id: impl Into<CameraId>,
        detections: impl Iterator<Item = &'a Detection> + Clone + 'a,
        timestamp: DateTime<Utc>,
    ) -> Vec<f64>;

    fn update_and_check_collision(&mut self, new_pos: Position3D) -> Option<CollisionPoint3D>;
}

impl<Track, EL> TrackRunner<Track, EL>
where
    Track: ObjectTrack,
    EL: EventLogger,
{
    pub fn new(camera_locs: Arc<CameraLocationStore>, event_logger: EL) -> Self {
        Self {
            tracks: HashMap::new(),
            missed_updates: HashMap::new(),
            camera_locs,
            id_gen: TrackIdGenerator::new(),
            event_logger,
        }
    }

    pub async fn start(
        mut self,
        mut pair_rx: mpsc::Receiver<PairedFrames>,
        collision_tx: mpsc::Sender<CollisionPoint3D>,
    ) -> anyhow::Result<()> {
        loop {
            let pair = pair_rx
                .recv()
                .await
                .with_context(|| "pair channel has been unexpectedly closed")
                .unwrap();
            let (collisions, events) = self.update_frame(pair);
            for coll in collisions {
                collision_tx
                    .send(coll)
                    .await
                    .with_context(|| "collision channel has been unexpectedly closed")
                    .unwrap();
            }
            self.event_logger.push_events(events);
        }
    }

    fn update_frame(&mut self, pair: PairedFrames) -> (Vec<CollisionPoint3D>, TrackingEventsDto) {
        let assignments = associate_objects(&mut self.tracks, &pair);
        let mut events = Vec::new();
        let res = self.update_assigned_tracks(&pair, &assignments);
        for track_id in res.collisions.keys() {
            trace!(?track_id, "detected collision");
            self.tracks.remove(track_id);
            self.missed_updates.remove(track_id);
        }
        events.push(TrackingEventBodyDto::UpdateTrack(res.clone()));

        let mut dropped_tracks = Vec::new();
        for (track_id, _) in &assignments {
            if !self.tracks.contains_key(track_id) {
                continue;
            }
            if res.assigned_tracks.contains(track_id) {
                self.missed_updates.remove(track_id);
                continue;
            }

            let missed_updates = self.missed_updates.entry(*track_id).or_default();
            *missed_updates += 1;
            if *missed_updates >= MAX_MISSED_UPDATES {
                dropped_tracks.push(*track_id);
            }
        }
        for track_id in dropped_tracks {
            self.tracks.remove(&track_id);
            self.missed_updates.remove(&track_id);
            events.push(TrackingEventBodyDto::DropTrack(track_id));
        }

        let (new_tracks, diagnostics): (Vec<Track>, _) = create_new_tracks(
            &self.id_gen,
            &res.assigned_dets_a,
            &res.assigned_dets_b,
            &pair,
            self.camera_locs.clone(),
        );
        events.push(TrackingEventBodyDto::NewTrackDiagnostics(diagnostics));
        for track in new_tracks {
            let track_id = track.id();
            self.tracks.insert(track_id, track);
            self.missed_updates.remove(&track_id);
            events.push(TrackingEventBodyDto::NewTrack);
        }

        let collisions = res.collisions.into_values().collect();
        (
            collisions,
            TrackingEventsDto {
                timestamp: pair.timestamp_avr,
                events,
            },
        )
    }

    fn update_assigned_tracks(
        &mut self,
        pair: &PairedFrames,
        assignments: &HashMap<TrackId, (Option<usize>, Option<usize>)>,
    ) -> AssignedTrackResult {
        self.event_logger.push_pair(pair);
        let mut assigned_dets_a = Vec::new();
        let mut assigned_dets_b = Vec::new();
        let mut assigned_tracks = Vec::new();
        let mut collisions = HashMap::new();

        let camera_a = self.camera_locs.get(pair.a.camera_id.into()).unwrap();
        let camera_b = self.camera_locs.get(pair.b.camera_id.into()).unwrap();
        for (track_id, (detection_a, detection_b)) in assignments {
            let (Some(detection_a), Some(detection_b)) = (*detection_a, *detection_b) else {
                continue;
            };
            let Ok(triangulation) = triangulate(
                camera_a.clone(),
                camera_a.rotate_to_world(pair.a.detections[detection_a].get_lay()),
                camera_b.clone(),
                camera_b.rotate_to_world(pair.b.detections[detection_b].get_lay()),
            ) else {
                continue;
            };
            if triangulation.ray_distance > NEW_TRACK_RAY_DISTANCE_GATE {
                continue;
            }

            assigned_dets_a.push(detection_a);
            assigned_dets_b.push(detection_b);
            assigned_tracks.push(*track_id);
            let track = self.tracks.get_mut(track_id).unwrap();
            if let Some(collision) = track.update_and_check_collision(triangulation.position) {
                collisions.insert(*track_id, collision);
            }
        }

        AssignedTrackResult {
            assigned_dets_a,
            assigned_dets_b,
            assigned_tracks,
            collisions,
        }
    }
}

fn create_new_tracks<Track>(
    id_gen: &TrackIdGenerator,
    assigned_dets_a: &[usize],
    assigned_dets_b: &[usize],
    pair: &PairedFrames,
    camera_locs: Arc<CameraLocationStore>,
) -> (Vec<Track>, NewTrackDiagnosticsDto)
where
    Track: ObjectTrack,
{
    let assigned_dets_a = assigned_dets_a.iter().copied().collect::<HashSet<_>>();
    let remaining_dets_a = (0..pair.a.detections.len())
        .filter(|idx| !assigned_dets_a.contains(idx))
        .collect::<Vec<_>>();
    let assigned_dets_b = assigned_dets_b.iter().copied().collect::<HashSet<_>>();
    let remaining_dets_b = (0..pair.b.detections.len())
        .filter(|idx| !assigned_dets_b.contains(idx))
        .collect::<Vec<_>>();

    let camera_a = camera_locs.get(pair.a.camera_id.into()).unwrap();
    let camera_b = camera_locs.get(pair.b.camera_id.into()).unwrap();
    let mut diagnostics = NewTrackDiagnosticsDto {
        camera_location_a: camera_a.clone(),
        camera_location_b: camera_b.clone(),
        unmatched_detections_a: remaining_dets_a.len(),
        unmatched_detections_b: remaining_dets_b.len(),
        candidate_pairs: remaining_dets_a.len() * remaining_dets_b.len(),
        ..Default::default()
    };
    if remaining_dets_a.is_empty() || remaining_dets_b.is_empty() {
        return (Vec::new(), diagnostics);
    }

    let mut costs = CostMatrix::filled(
        remaining_dets_a.len(),
        remaining_dets_b.len(),
        NEW_TRACK_RAY_DISTANCE_GATE + 1.0,
    );

    for (row, detection_a) in remaining_dets_a.iter().enumerate() {
        for (column, detection_b) in remaining_dets_b.iter().enumerate() {
            match triangulate(
                camera_a.clone(),
                camera_a.rotate_to_world(pair.a.detections[*detection_a].get_lay()),
                camera_b.clone(),
                camera_b.rotate_to_world(pair.b.detections[*detection_b].get_lay()),
            ) {
                Ok(triangulation) => {
                    diagnostics.min_ray_distance = Some(
                        diagnostics
                            .min_ray_distance
                            .map_or(triangulation.ray_distance, |current| {
                                current.min(triangulation.ray_distance)
                            }),
                    );
                    if triangulation.ray_distance <= NEW_TRACK_RAY_DISTANCE_GATE {
                        diagnostics.within_gate_candidates += 1;
                    } else {
                        diagnostics.ray_distance_rejections += 1;
                    }
                    costs.set(row, column, triangulation.ray_distance);
                }
                Err(TriangulationError::ParallelRays) => diagnostics.parallel_ray_rejections += 1,
                Err(TriangulationError::IntersectionBehindCamera) => {
                    diagnostics.behind_camera_rejections += 1
                }
            }
        }
    }

    let assignment = hungarian_gated(&costs, NEW_TRACK_RAY_DISTANCE_GATE).unwrap();
    let tracks: Vec<Track> = assignment
        .pairs()
        .filter_map(|(row, column)| {
            let detection_a = remaining_dets_a[row];
            let detection_b = remaining_dets_b[column];
            let triangulation = triangulate(
                camera_a.clone(),
                camera_a.rotate_to_world(pair.a.detections[detection_a].get_lay()),
                camera_b.clone(),
                camera_b.rotate_to_world(pair.b.detections[detection_b].get_lay()),
            )
            .ok()?;
            Some(Track::new(
                id_gen.next(),
                Vector3::new(
                    triangulation.position.x,
                    triangulation.position.y,
                    triangulation.position.z,
                ),
                pair.timestamp_avr,
                camera_locs.clone(),
            ))
        })
        .collect();

    diagnostics.accepted_tracks = tracks.len();
    diagnostics.unselected_within_gate_candidates = diagnostics
        .within_gate_candidates
        .saturating_sub(tracks.len());
    (tracks, diagnostics)
}

/// Result of [`TrackRunner::update_assigned_tracks()`]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignedTrackResult {
    assigned_dets_a: Vec<usize>,
    assigned_dets_b: Vec<usize>,
    assigned_tracks: Vec<TrackId>,
    collisions: HashMap<TrackId, CollisionPoint3D>,
}

struct TrackIdGenerator {
    next: Cell<usize>,
}

impl TrackIdGenerator {
    pub fn new() -> Self {
        Self { next: Cell::new(0) }
    }

    pub fn next(&self) -> TrackId {
        let next = self.next.get();
        self.next.set(next + 1);
        TrackId(next)
    }
}

/// Newtype to represent unique track id.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackId(usize);

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;
    use struckout_proto::{CameraLocation, DetectionsPacket};

    use super::*;

    #[derive(Debug)]
    struct StubTrack {
        id: TrackId,
        initial_position: Vector3<f64>,
    }

    impl ObjectTrack for StubTrack {
        fn new(
            id: TrackId,
            initial_position: Vector3<f64>,
            _timestamp: DateTime<Utc>,
            _camera_loc_provider: Arc<CameraLocationStore>,
        ) -> Self {
            Self {
                id,
                initial_position,
            }
        }

        fn id(&self) -> TrackId {
            self.id
        }

        fn evaluate_scores<'a>(
            &mut self,
            _camera_id: impl Into<CameraId>,
            detections: impl Iterator<Item = &'a Detection> + Clone + 'a,
            _timestamp: DateTime<Utc>,
        ) -> Vec<f64> {
            detections.map(|_| 0.0).collect()
        }

        fn update_and_check_collision(&mut self, _new_pos: Position3D) -> Option<CollisionPoint3D> {
            None
        }
    }

    struct NullEventLogger;

    impl EventLogger for NullEventLogger {
        fn push_events(&mut self, _events: TrackingEventsDto) {}

        fn push_pair(&mut self, _pair: &PairedFrames) {}
    }

    fn camera_locations() -> Arc<CameraLocationStore> {
        let locations = Arc::new(CameraLocationStore::new());
        locations.insert(
            CameraId::new(0),
            CameraLocation {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                ..Default::default()
            },
        );
        locations.insert(
            CameraId::new(1),
            CameraLocation {
                x: 0.0,
                y: 10.0,
                z: 0.0,
                ..Default::default()
            },
        );
        locations
    }

    fn detection(direction: Vector3<f64>) -> Detection {
        Detection {
            bbox_width: 10,
            bbox_height: 10,
            lay_x: direction.x,
            lay_y: direction.y,
            lay_z: direction.z,
        }
    }

    fn pair(detections_a: Vec<Vector3<f64>>, detections_b: Vec<Vector3<f64>>) -> PairedFrames {
        PairedFrames {
            timestamp_avr: DateTime::default(),
            a: DetectionsPacket {
                camera_id: 0,
                session_id: "a".to_string(),
                timestamp: 0,
                frame_id: 1,
                detections: detections_a.into_iter().map(detection).collect(),
            },
            b: DetectionsPacket {
                camera_id: 1,
                session_id: "b".to_string(),
                timestamp: 0,
                frame_id: 1,
                detections: detections_b.into_iter().map(detection).collect(),
            },
        }
    }

    #[test]
    fn creates_tracks_by_geometric_correspondence() {
        let target_1 = Vector3::new(100.0, 2.0, 0.0);
        let target_2 = Vector3::new(100.0, 8.0, 20.0);
        let frame = pair(
            vec![target_1, target_2],
            vec![
                target_2 - Vector3::new(0.0, 10.0, 0.0),
                target_1 - Vector3::new(0.0, 10.0, 0.0),
            ],
        );

        let (tracks, diagnostics): (Vec<StubTrack>, _) = create_new_tracks(
            &TrackIdGenerator::new(),
            &[],
            &[],
            &frame,
            camera_locations(),
        );

        assert_eq!(diagnostics.candidate_pairs, 4);
        assert_eq!(diagnostics.accepted_tracks, 2);
        assert_relative_eq!(diagnostics.min_ray_distance.unwrap(), 0.0);
        assert_eq!(tracks.len(), 2);
        assert_relative_eq!(tracks[0].initial_position.x, target_1.x);
        assert_relative_eq!(tracks[0].initial_position.y, target_1.y);
        assert_relative_eq!(tracks[0].initial_position.z, target_1.z);
        assert_relative_eq!(tracks[1].initial_position.x, target_2.x);
        assert_relative_eq!(tracks[1].initial_position.y, target_2.y);
        assert_relative_eq!(tracks[1].initial_position.z, target_2.z);
    }

    #[test]
    fn creates_track_after_rotating_device_rays_into_world_coordinates() {
        let locations = camera_locations();
        locations.insert(
            CameraId::new(1),
            CameraLocation {
                x: 0.0,
                y: 10.0,
                z: 0.0,
                rotation_z_degrees: 90.0,
                ..Default::default()
            },
        );
        let target = Vector3::new(100.0, 2.0, 0.0);
        let camera_b_device_direction = Vector3::new(-8.0, -100.0, 0.0);
        let frame = pair(vec![target], vec![camera_b_device_direction]);

        let (tracks, diagnostics): (Vec<StubTrack>, _) =
            create_new_tracks(&TrackIdGenerator::new(), &[], &[], &frame, locations);

        assert_eq!(diagnostics.candidate_pairs, 1);
        assert_eq!(diagnostics.behind_camera_rejections, 0);
        assert_eq!(diagnostics.accepted_tracks, 1);
        assert_eq!(tracks.len(), 1);
        assert_relative_eq!(tracks[0].initial_position.x, target.x, epsilon = 1e-9);
        assert_relative_eq!(tracks[0].initial_position.y, target.y, epsilon = 1e-9);
        assert_relative_eq!(tracks[0].initial_position.z, target.z, epsilon = 1e-9);
    }

    #[test]
    fn creates_only_matched_tracks_when_detection_counts_differ() {
        let target = Vector3::new(100.0, 2.0, 0.0);
        let frame = pair(
            vec![target, Vector3::new(20.0, 30.0, 40.0)],
            vec![target - Vector3::new(0.0, 10.0, 0.0)],
        );

        let (tracks, diagnostics): (Vec<StubTrack>, _) = create_new_tracks(
            &TrackIdGenerator::new(),
            &[],
            &[],
            &frame,
            camera_locations(),
        );

        assert_eq!(diagnostics.candidate_pairs, 2);
        assert_eq!(diagnostics.accepted_tracks, 1);
        assert_eq!(tracks.len(), 1);
        assert_relative_eq!(tracks[0].initial_position.x, target.x);
        assert_relative_eq!(tracks[0].initial_position.y, target.y);
    }

    #[test]
    fn reports_triangulation_rejection_reasons() {
        let frame = pair(
            vec![Vector3::new(1.0, 0.0, 0.0)],
            vec![Vector3::new(1.0, 0.0, 0.0), Vector3::new(0.0, 1.0, 0.0)],
        );

        let (tracks, diagnostics): (Vec<StubTrack>, _) = create_new_tracks(
            &TrackIdGenerator::new(),
            &[],
            &[],
            &frame,
            camera_locations(),
        );

        assert!(tracks.is_empty());
        assert_eq!(diagnostics.candidate_pairs, 2);
        assert_eq!(diagnostics.parallel_ray_rejections, 1);
        assert_eq!(diagnostics.behind_camera_rejections, 1);
        assert_eq!(diagnostics.ray_distance_rejections, 0);
        assert!(diagnostics.min_ray_distance.is_none());
        assert_eq!(diagnostics.accepted_tracks, 0);
    }

    #[test]
    fn reports_candidates_rejected_by_ray_distance() {
        let locations = camera_locations();
        locations.insert(
            CameraId::new(1),
            CameraLocation {
                x: 0.0,
                y: 100.0,
                z: 0.0,
                ..Default::default()
            },
        );
        let frame = pair(
            vec![Vector3::new(1.0, 0.0, 0.0)],
            vec![Vector3::new(1.0, 0.0, 1.0)],
        );

        let (tracks, diagnostics): (Vec<StubTrack>, _) =
            create_new_tracks(&TrackIdGenerator::new(), &[], &[], &frame, locations);

        assert!(tracks.is_empty());
        assert_eq!(diagnostics.candidate_pairs, 1);
        assert_eq!(diagnostics.parallel_ray_rejections, 0);
        assert_eq!(diagnostics.behind_camera_rejections, 0);
        assert_eq!(diagnostics.ray_distance_rejections, 1);
        assert_eq!(diagnostics.within_gate_candidates, 0);
        assert_eq!(diagnostics.accepted_tracks, 0);
        assert_relative_eq!(diagnostics.min_ray_distance.unwrap(), 100.0);
    }

    #[test]
    fn keeps_a_track_until_the_missed_update_limit() {
        let locations = camera_locations();
        let mut runner = TrackRunner::new(locations.clone(), NullEventLogger);
        let track_id = runner.id_gen.next();
        runner.tracks.insert(
            track_id,
            StubTrack::new(track_id, Vector3::zeros(), DateTime::default(), locations),
        );

        for _ in 0..MAX_MISSED_UPDATES - 1 {
            let (_, events) = runner.update_frame(pair(Vec::new(), Vec::new()));
            assert!(runner.tracks.contains_key(&track_id));
            assert_eq!(events.events.len(), 2);
        }

        let (_, events) = runner.update_frame(pair(Vec::new(), Vec::new()));
        assert!(!runner.tracks.contains_key(&track_id));
        assert!(matches!(events.events[1], TrackingEventBodyDto::DropTrack(id) if id == track_id));
    }
}
