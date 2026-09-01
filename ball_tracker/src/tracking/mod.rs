use std::{cell::Cell, collections::HashMap, sync::Arc};

use crate::{
    CameraLocationStore,
    detection_input::PairedFrames,
    tracking::{data_association::associate_objects, triangulate::triangulate},
    types::{CameraId, CollisionPoint3D, GetLayFromDetection as _, Position3D, ToVector3},
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

pub struct TrackRunner<T, EL> {
    tracks: HashMap<TrackId, T>,
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
        // known tracks
        let res = self.update_assigned_tracks(&pair, &assignments);
        for (track_id, _) in &res.collisions {
            trace!(?track_id, "detected collision");
            self.tracks.remove(track_id);
        }
        events.push(TrackingEventBodyDto::UpdateTrack(res.clone())); // OPTIM: 無駄clone

        // dropped tracks
        assignments
            .iter()
            .filter_map(|(track, (a, b))| {
                if a.is_none() && b.is_none() {
                    Some(track)
                } else {
                    None
                }
            })
            .for_each(|track_id| {
                // FIXME: 数フレーム待ってから削除すると良いかも
                self.tracks.remove(track_id);
                events.push(TrackingEventBodyDto::DropTrack(*track_id));
            });

        // new track
        let new_tracks: Vec<Track> =
            create_new_tracks(&self.id_gen, &assignments, &pair, self.camera_locs.clone());
        for t in new_tracks {
            self.tracks.insert(t.id(), t);
            events.push(TrackingEventBodyDto::NewTrack);
        }

        let colls = res.collisions.into_iter().map(|(_, coll)| coll).collect();
        (
            colls,
            TrackingEventsDto {
                timestamp: pair.timestamp_avr,
                events,
            },
        )
    }

    /// Updates tracks based on assignment.
    fn update_assigned_tracks(
        &mut self,
        pair: &PairedFrames,
        assignments: &HashMap<TrackId, (Option<usize>, Option<usize>)>,
    ) -> AssignedTrackResult {
        let mut assigned_dets_a = Vec::new();
        let mut assigned_dets_b = Vec::new();
        let mut assigned_tracks = Vec::new();
        let mut collisions = HashMap::new();
        assignments
            .iter()
            .filter_map(|(track, (a, b))| {
                if a.is_some() && b.is_some() {
                    Some((track, (a.unwrap(), b.unwrap())))
                } else {
                    None
                }
            })
            .for_each(|(track_id, (det_a, det_b))| {
                assigned_dets_a.push(det_a);
                assigned_dets_b.push(det_b);
                assigned_tracks.push(*track_id);
                let new_pos = triangulate(
                    self.camera_locs.get(CameraId::new(0)).unwrap().clone(),
                    pair.a.detections.get(det_a).unwrap().get_lay(),
                    self.camera_locs.get(CameraId::new(1)).unwrap().clone(),
                    pair.b.detections.get(det_b).unwrap().get_lay(),
                );
                let track = self.tracks.get_mut(track_id).unwrap();
                let coll = track.update_and_check_collision(new_pos);
                if let Some(coll) = coll {
                    collisions.insert(*track_id, coll);
                }
            });
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
    assignments: &HashMap<TrackId, (Option<usize>, Option<usize>)>,
    pair: &PairedFrames,
    camera_locs: Arc<CameraLocationStore>,
) -> Vec<Track>
where
    Track: ObjectTrack,
{
    let assigned_dets_a = assignments
        .iter()
        .filter_map(|(_, (a, _))| *a)
        .collect::<Vec<_>>();
    let remaining_dets_a = pair
        .a
        .detections
        .iter()
        .enumerate()
        .filter_map(|(idx, _)| {
            if assigned_dets_a.contains(&idx) {
                Some(idx)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let assigned_dets_b = assignments
        .iter()
        .filter_map(|(_, (_, b))| *b)
        .collect::<Vec<_>>();
    let remaining_dets_b = pair
        .b
        .detections
        .iter()
        .enumerate()
        .filter_map(|(idx, _)| {
            if assigned_dets_b.contains(&idx) {
                Some(idx)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(remaining_dets_a.len(), remaining_dets_b.len());

    remaining_dets_a
        .iter()
        .zip(remaining_dets_b.iter())
        .map(|(det_a, det_b)| {
            let loc_a = camera_locs.get(pair.a.camera_id.into()).unwrap();
            let lay_a = pair.a.detections[*det_a].get_lay();
            let loc_b = camera_locs.get(pair.b.camera_id.into()).unwrap();
            let lay_b = pair.b.detections[*det_b].get_lay();
            let pos = triangulate(loc_a, lay_a, loc_b, lay_b);
            let next_id = id_gen.next();
            Track::new(
                next_id,
                pos.to_vector3(),
                pair.timestamp_avr,
                camera_locs.clone(),
            )
        })
        .collect()
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
mod tests {}
