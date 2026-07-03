use std::{collections::HashMap, sync::Arc};

use crate::{
    detection_input::PairedFrames,
    tracking::data_association::associate_objects,
    types::{CameraId, CollisionPoint3D, GetLayFromDetectedObject as _, Position3D},
};

mod data_association;
mod kalman;
mod triangulate;
use anyhow::Context;
use chrono::{DateTime, Utc};
pub use kalman::KalmanTrack;
use parking_lot::RwLock;
use struckout_proto::{CameraLocation, DetectedObject};
use tokio::sync::mpsc;

pub struct TrackRunner<T, P> {
    tracks: Vec<T>,
    camera_loc_provider: P,
}

/// Tracks an object.
pub trait ObjectTrack {
    /// Predict object location and evaluate scores for each detections.
    fn evaluate_scores<'a>(
        &mut self,
        camera_id: impl Into<CameraId>,
        detections: impl Iterator<Item = &'a DetectedObject> + Clone + 'a,
        timestamp: DateTime<Utc>,
    ) -> Vec<f64>;

    fn update_and_check_collision(&mut self, new_pos: Position3D) -> Option<CollisionPoint3D>;
}

pub trait CameraLocationProvider: Send + 'static + Clone {
    fn get(&self, id: CameraId) -> Option<CameraLocation>;
}

impl CameraLocationProvider for Arc<RwLock<crate::State>> {
    fn get(&self, id: CameraId) -> Option<CameraLocation> {
        self.read().camera_locs.get(&id).cloned()
    }
}

impl<T, P> TrackRunner<T, P>
where
    T: ObjectTrack,
    P: CameraLocationProvider,
{
    pub fn new(camera_loc_provider: P) -> Self {
        Self {
            tracks: Vec::new(),
            camera_loc_provider,
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
        }
    }

    pub fn update_frame(&mut self, pair: PairedFrames) -> Vec<CollisionPoint3D> {
        let assignments = associate_objects(&mut self.tracks, &pair);
        // known tracks
        let res = self.update_assigned_tracks(&pair, &assignments);
        for (track_idx, _) in &res.collisions {
            self.tracks.remove(*track_idx);
        }

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
            .for_each(|track_idx| todo!("dropped track: {}", track_idx));

        // new track
        // TODO

        res.collisions.into_iter().map(|(_, coll)| coll).collect()
    }

    /// Updates tracks based on assignment.
    pub fn update_assigned_tracks(
        &mut self,
        pair: &PairedFrames,
        assignments: &HashMap<usize, (Option<usize>, Option<usize>)>,
    ) -> UpdateTrackResult {
        let mut assigned_dets_a = Vec::new();
        let mut assigned_dets_b = Vec::new();
        let mut collisions = Vec::new();
        assignments
            .iter()
            .filter_map(|(track, (a, b))| {
                if a.is_some() && b.is_some() {
                    Some((track, (a.unwrap(), b.unwrap())))
                } else {
                    None
                }
            })
            .for_each(|(track_idx, (det_a, det_b))| {
                assigned_dets_a.push(det_a);
                assigned_dets_b.push(det_b);
                let new_pos = triangulate::triangulate(
                    self.camera_loc_provider
                        .get(CameraId::new(0))
                        .unwrap()
                        .clone(),
                    pair.a.detected_objects.get(det_a).unwrap().get_lay(),
                    self.camera_loc_provider
                        .get(CameraId::new(1))
                        .unwrap()
                        .clone(),
                    pair.b.detected_objects.get(det_b).unwrap().get_lay(),
                );
                let track = self.tracks.get_mut(*track_idx).unwrap();
                let coll = track.update_and_check_collision(new_pos);
                if let Some(coll) = coll {
                    collisions.push((*track_idx, coll));
                }
            });
        UpdateTrackResult {
            assigned_dets_a,
            assigned_dets_b,
            collisions,
        }
    }
}

/// Result of [`State::update_assigned_tracks()`]
#[derive(Debug)]
pub struct UpdateTrackResult {
    assigned_dets_a: Vec<usize>,
    assigned_dets_b: Vec<usize>,
    /// (`track_idx`, `collision_point`)
    collisions: Vec<(usize, CollisionPoint3D)>,
}

#[cfg(test)]
mod tests {}
