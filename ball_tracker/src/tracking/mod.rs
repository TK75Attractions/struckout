use std::{
    cell::Cell,
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    sync::Arc,
    time::Duration,
};

use crate::{
    CameraLocationStore,
    clock_sync::AutoTimeTrim,
    detection_input::CameraFrame,
    tracking::{
        ball_track::{
            BallTrack, RAY_LIFETIME, REPORT_LEAD, REPORT_REFRACTORY, REPORTED_TRACK_LINGER,
            TRACK_IDLE_TIMEOUT, seed_tracks,
        },
        ray_buffer::{BufferedRay, RayBuffer, RayId},
        trajectory::{BallisticTrajectory, RayObservation},
    },
    types::{
        CameraId, CollisionPoint3D, GetLayFromDetection as _, RotateToWorld as _, ToVector3 as _,
    },
};

mod ball_track;
pub use ball_track::{RejectReason, SeedDiagnostics};
mod event;
pub use event::*;
mod ray_buffer;
mod trajectory;
mod triangulate;

use anyhow::Context;
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use struckout_proto::{CameraPose, Detection};
use tokio::{sync::mpsc, time::MissedTickBehavior};
use tracing::{trace, warn};

/// 予測した着弾時刻が来たか、古くなったトラックがないかを確かめる間隔。
const COLLISION_POLL_INTERVAL: Duration = Duration::from_millis(5);
/// 直近の何球ぶんの残差で、時刻やカメラの位置のずれを疑うか。
const RESIDUAL_HISTORY: usize = 10;
/// 直近の投球の「視線と軌道の距離」の中央値がこれを超えたら警告する。
/// 2026-10-10 の録画では、時刻が合っていれば 1.4cm、25ms ずれると 3.8cm だった。
const RESIDUAL_WARNING: f64 = 0.03;
/// 2 台の時刻のずれを探す範囲 (ms)。
const TIME_SHIFT_SEARCH_MS: std::ops::RangeInclusive<i64> = -30..=60;

/// 届いたフレームの視線をためて、ボールのトラックを作り、着弾を報告する。
///
/// 手順は `docs/ball_detection_plan_2_track_creation.md` の 3.2〜3.5 にある。
pub struct TrackRunner<EL> {
    /// 追跡中のトラックと、報告したあとしばらく残しているトラック。
    tracks: Vec<TrackEntry>,
    buffer: RayBuffer,
    /// 最後に報告した着弾の時刻。直後の跳ね返りや影を、別の着弾として報告しないために使う。
    last_report: Option<DateTime<Utc>>,
    /// 直近に報告した投球の、視線と軌道の距離の二乗平均平方根。
    recent_residuals: VecDeque<f64>,
    camera_locs: Arc<CameraLocationStore>,
    id_gen: TrackIdGenerator,
    event_logger: EL,
    /// 予測した着弾時刻のどれだけ前に報告するか。負なら、着弾のあとに報告する
    report_lead: TimeDelta,
    /// 報告した投球の時刻合わせの手がかりを渡す先 (右のカメラの時刻の自動の補正)
    auto_trim: Option<Arc<AutoTimeTrim>>,
}

struct TrackEntry {
    ball: BallTrack,
    /// このトラックを作ったフレームの最初の視線。これより前の視線は、トラックができる前に届いていた。
    seeded_at: RayId,
    /// 予測した着弾。ボールは的に届く前にカメラの視野から出るので、見えているうちに予測しておき、
    /// その時刻が近づいてから報告する。
    pending: Option<PredictedCollision>,
    /// 最後に視線が増えた時刻 (この PC の時計)。
    updated: DateTime<Utc>,
    /// 着弾を報告した (または重複として報告を見送った)。同じボールの残りの視線を引き取るために
    /// [`REPORTED_TRACK_LINGER`] だけ残す。
    reported: bool,
}

/// 軌道から予測した、的に当たる時刻と位置。
#[derive(Debug, Clone, Copy)]
pub struct PredictedCollision {
    pub time: DateTime<Utc>,
    pub point: CollisionPoint3D,
}

impl<EL> TrackRunner<EL>
where
    EL: EventLogger,
{
    pub fn new(camera_locs: Arc<CameraLocationStore>, event_logger: EL) -> Self {
        Self {
            tracks: Vec::new(),
            buffer: RayBuffer::new(),
            last_report: None,
            recent_residuals: VecDeque::new(),
            camera_locs,
            id_gen: TrackIdGenerator::new(),
            event_logger,
            report_lead: REPORT_LEAD,
            auto_trim: None,
        }
    }

    /// 報告した投球の時刻合わせの手がかりを、右のカメラの時刻の自動の補正に渡す。
    pub fn with_auto_trim(mut self, auto_trim: Arc<AutoTimeTrim>) -> Self {
        self.auto_trim = Some(auto_trim);
        self
    }

    /// 着弾を、予測した着弾時刻の `delay` 後に報告する (負なら前)。既定は [`REPORT_LEAD`] 前。
    ///
    /// 遅らせるほど、着弾の直前に撮ったフレーム (ボールが的に近く、大きく写っている) が報告に間に合い、
    /// 着弾位置の予測が正確になる。代わりに、的への描画がそのぶん着弾より遅れる。
    pub fn with_report_delay(mut self, delay: TimeDelta) -> Self {
        self.report_lead = -delay;
        self
    }

    pub async fn start(
        mut self,
        mut frame_rx: mpsc::Receiver<CameraFrame>,
        collision_tx: mpsc::Sender<CollisionPoint3D>,
    ) -> anyhow::Result<()> {
        // カメラは検出があったフレームしか送ってこないので、フレームが届くのを待っているだけでは
        // 着弾時刻を逃す。フレームとは別に、時計でも着弾時刻を確かめる。
        let mut poll = tokio::time::interval(COLLISION_POLL_INTERVAL);
        poll.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            let (collisions, events) = tokio::select! {
                frame = frame_rx.recv() => {
                    let frame = frame
                        .with_context(|| "frame channel has been unexpectedly closed")
                        .unwrap();
                    self.update_frame(frame, Utc::now())
                }
                _ = poll.tick() => self.poll_collisions(Utc::now()),
            };
            for coll in collisions {
                collision_tx
                    .send(coll)
                    .await
                    .with_context(|| "collision channel has been unexpectedly closed")
                    .unwrap();
            }
            if !events.events.is_empty() {
                self.event_logger.push_events(events);
            }
        }
    }

    /// フレームが届いていない間も、着弾の報告とトラックの片付けを進める。
    ///
    /// `now` は [`CameraFrame`] の時刻と同じ時計 (この PC の時計) で渡す。録画を解析し直すとき
    /// (`bin/replay_video.rs`) は、[`Self::start`] の代わりにこれと [`Self::update_frame`] を直接呼ぶ。
    pub fn poll_collisions(&mut self, now: DateTime<Utc>) -> (Vec<CollisionPoint3D>, TrackingEventsDto) {
        let mut events = Vec::new();
        let collisions = self.take_due_collisions(now, &mut events);
        self.expire(now, &mut events);
        (
            collisions,
            TrackingEventsDto {
                timestamp: now,
                events,
            },
        )
    }

    /// 1 台のカメラの 1 フレームぶんの検出で、追跡を進める。`now` はこの PC の時計での今の時刻。
    pub fn update_frame(
        &mut self,
        frame: CameraFrame,
        now: DateTime<Utc>,
    ) -> (Vec<CollisionPoint3D>, TrackingEventsDto) {
        self.event_logger.push_frame(&frame);
        let mut events = Vec::new();
        // 着弾したはずのトラックは、このフレームの視線を足す前に報告する。
        let mut collisions = self.take_due_collisions(now, &mut events);
        self.expire(now, &mut events);

        let camera_id = CameraId::new(frame.packet.camera_id);
        if let Some(camera) = self.camera_locs.get(camera_id) {
            let rays = frame
                .packet
                .detections
                .iter()
                .map(|detection| ray(&camera, detection, frame.detection_time(detection)));
            let new = self.buffer.push_frame(camera_id, frame.time, rays);
            self.extend_tracks(&new, now, &mut events);
            self.create_tracks(&new, now, &mut events);
            // 的のすぐ手前まで見えていたボールは、この時点でもう報告する時刻になっている。
            collisions.extend(self.take_due_collisions(now, &mut events));
        } else {
            warn!(
                camera_id = frame.packet.camera_id,
                "received a frame from a camera whose pose is unknown"
            );
        }

        (
            collisions,
            TrackingEventsDto {
                timestamp: now,
                events,
            },
        )
    }

    /// 報告する時刻 (既定は予測した着弾の [`REPORT_LEAD`] 前) が来た着弾を、予測時刻の早い順に取り出す。
    ///
    /// 1 個のボールを 2 回報告しないよう、直前の報告に近いものと、同じころに着弾する、
    /// よりボールらしいトラックがあるものは報告しない。
    fn take_due_collisions(
        &mut self,
        now: DateTime<Utc>,
        events: &mut Vec<TrackingEventBodyDto>,
    ) -> Vec<CollisionPoint3D> {
        let mut reported = HashMap::new();
        let mut diagnostics = Vec::new();
        loop {
            let due = self
                .tracks
                .iter()
                .enumerate()
                .filter(|(_, track)| !track.reported)
                .filter_map(|(index, track)| track.pending.map(|pending| (index, pending)))
                .filter(|(_, pending)| pending.time - self.report_lead <= now)
                .min_by_key(|(_, pending)| pending.time);
            let Some((index, pending)) = due else {
                break;
            };
            self.tracks[index].reported = true;
            let track = &self.tracks[index];
            let track_id = track.ball.id();
            let outranked = self.tracks.iter().any(|other| {
                !other.reported
                    && other
                        .pending
                        .is_some_and(|p| (p.time - pending.time).abs() <= REPORT_REFRACTORY)
                    && other.ball.outranks(&track.ball)
            });
            let too_soon = self
                .last_report
                .is_some_and(|last| (pending.time - last).abs() <= REPORT_REFRACTORY);
            if too_soon || outranked {
                events.push(TrackingEventBodyDto::SuppressedCollision {
                    track_id,
                    reason: if too_soon {
                        SuppressReason::TooSoonAfterReport
                    } else {
                        SuppressReason::OutrankedByAnotherTrack
                    },
                });
                continue;
            }
            trace!(?track_id, "detected collision");
            self.last_report = Some(pending.time);
            reported.insert(track_id, pending.point);
            diagnostics.extend(throw_diagnostics(track_id, track.ball.rays()));
        }
        if !reported.is_empty() {
            events.push(TrackingEventBodyDto::Collision(reported.clone()));
        }
        for diagnostics in diagnostics {
            self.watch_residual(diagnostics.residual_rms);
            if let (Some(auto_trim), Some(right)) = (&self.auto_trim, self.camera_locs.rightmost()) {
                auto_trim.observe(right.value(), &diagnostics);
            }
            events.push(TrackingEventBodyDto::ThrowDiagnostics(diagnostics));
        }
        reported.into_values().collect()
    }

    /// 視線と軌道の距離が大きい投球が続いたら警告する。2 台の時刻か、カメラの位置と向きがずれている。
    fn watch_residual(&mut self, residual_rms: f64) {
        if self.recent_residuals.len() >= RESIDUAL_HISTORY {
            self.recent_residuals.pop_front();
        }
        self.recent_residuals.push_back(residual_rms);
        if self.recent_residuals.len() < RESIDUAL_HISTORY {
            return;
        }
        let mut sorted = self.recent_residuals.iter().copied().collect::<Vec<_>>();
        sorted.sort_by(f64::total_cmp);
        let median = sorted[sorted.len() / 2];
        if median > RESIDUAL_WARNING {
            warn!(
                median_residual_cm = median * 100.0,
                "rays do not fit the trajectories well; check the camera clocks and poses"
            );
        }
    }

    /// 古い視線と、視線が増えなくなったトラックを捨てる。
    fn expire(&mut self, now: DateTime<Utc>, events: &mut Vec<TrackingEventBodyDto>) {
        self.buffer.prune(now - RAY_LIFETIME);
        let buffer = &mut self.buffer;
        self.tracks.retain(|track| {
            if track.reported {
                return track
                    .pending
                    .is_some_and(|pending| now <= pending.time + REPORTED_TRACK_LINGER);
            }
            if track.pending.is_none() && now - track.updated > TRACK_IDLE_TIMEOUT {
                // ボールではなかった。持っていた視線は、ほかのトラックが使えるように戻す。
                for ray in track.ball.rays() {
                    buffer.set_owner(ray.id, None);
                }
                events.push(TrackingEventBodyDto::DropTrack(track.ball.id()));
                return false;
            }
            true
        });
    }

    /// いまあるトラックに、軌道に最も近い視線を足す。
    ///
    /// 足す候補は、いま届いたフレームの視線と、トラックができる前に届いていてどのトラックにも
    /// 入っていない視線。後者を見ないと、片方のカメラのフレームが遅れて届いたときに、先に届いていた
    /// もう一方のカメラの視線を使い残す。
    fn extend_tracks(
        &mut self,
        new: &[BufferedRay],
        now: DateTime<Utc>,
        events: &mut Vec<TrackingEventBodyDto>,
    ) {
        let mut candidates = Vec::new();
        for (index, track) in self.tracks.iter().enumerate() {
            for ray in new {
                if let Some(distance) = track.ball.association_distance(ray) {
                    candidates.push((distance, index, *ray));
                }
            }
            if track.reported {
                continue;
            }
            for ray in self.buffer.free_rays() {
                if ray.id >= track.seeded_at {
                    continue;
                }
                if let Some(distance) = track.ball.association_distance(ray) {
                    candidates.push((distance, index, *ray));
                }
            }
        }
        // 時刻の早い視線から、同じ時刻なら近いものから試す。
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
        candidates.sort_by_key(|candidate| candidate.2.time());

        // 1 つのトラックが 1 フレームから取る視線は 1 本まで。いちばん近い視線が合わなければ、
        // そのフレームからは取らない。1 本の視線は 1 つのトラックにしか入らない。
        let mut tried = HashSet::new();
        let mut used_rays = HashSet::new();
        let mut extended = Vec::new();
        for (_, index, ray) in candidates {
            if used_rays.contains(&ray.id) || !tried.insert((index, ray.camera, ray.time())) {
                continue;
            }
            let track = &mut self.tracks[index];
            // 報告したトラックは軌道を変えず、同じボールの残りの視線を引き取るだけにする。
            if !track.reported {
                if !track.ball.try_extend(&ray) {
                    continue;
                }
                track.updated = now;
                if !extended.contains(&index) {
                    extended.push(index);
                }
            }
            used_rays.insert(ray.id);
            self.buffer.set_owner(ray.id, Some(track.ball.id()));
        }

        // フレームはあとから届くこともあるので、視線が増えていないトラックも確かめ直す。
        for (index, track) in self.tracks.iter_mut().enumerate() {
            if track.reported {
                continue;
            }
            let prediction = track.ball.predicted_collision(&self.buffer);
            if extended.contains(&index) {
                events.push(TrackingEventBodyDto::UpdateTrack(TrackSummaryDto::new(
                    &track.ball,
                    &prediction,
                )));
            }
            track.pending = prediction.ok();
        }
    }

    /// どのトラックにも入っていない視線から、新しいトラックを作る。
    fn create_tracks(
        &mut self,
        new: &[BufferedRay],
        now: DateTime<Utc>,
        events: &mut Vec<TrackingEventBodyDto>,
    ) {
        let Some(first) = new.first() else {
            return;
        };
        let (created, diagnostics) = seed_tracks(&mut self.buffer, new, &self.id_gen);
        if diagnostics.tested_seeds > 0 {
            events.push(TrackingEventBodyDto::SeedDiagnostics(diagnostics));
        }
        for ball in created {
            let prediction = ball.predicted_collision(&self.buffer);
            events.push(TrackingEventBodyDto::NewTrack(TrackSummaryDto::new(
                &ball,
                &prediction,
            )));
            self.tracks.push(TrackEntry {
                ball,
                seeded_at: first.id,
                pending: prediction.ok(),
                updated: now,
                reported: false,
            });
        }
    }
}

/// カメラがその検出を見た方向を、世界座標の視線にする。
fn ray(camera: &CameraPose, detection: &Detection, time: DateTime<Utc>) -> RayObservation {
    RayObservation {
        time,
        origin: camera.to_vector3(),
        direction: camera.rotate_to_world(detection.get_lay()),
    }
}

/// 報告した投球の視線から、2 台の時刻のずれの手がかりを出す。
///
/// 右のカメラ (位置の y が大きいほう) の時刻をずらしながら軌道を当てはめ直し、視線と軌道の距離が
/// 最も小さくなるずれを探す。このずれにはカメラの位置と向きの誤差も混ざるので、1 球の値では判断しない。
fn throw_diagnostics(track_id: TrackId, rays: &[BufferedRay]) -> Option<ThrowDiagnosticsDto> {
    let right = rays
        .iter()
        .max_by(|a, b| a.observation.origin.y.total_cmp(&b.observation.origin.y))?
        .camera;
    if rays.iter().all(|ray| ray.camera == right) {
        return None;
    }
    let residual = |shift_ms: i64| {
        let shifted = rays
            .iter()
            .map(|ray| {
                let mut observation = ray.observation;
                if ray.camera == right {
                    observation.time += TimeDelta::milliseconds(shift_ms);
                }
                observation
            })
            .collect::<Vec<_>>();
        BallisticTrajectory::fit(&shifted).map(|trajectory| trajectory.residual_rms)
    };
    let residual_rms = residual(0)?;
    let (best_time_shift_ms, best_residual_rms) = TIME_SHIFT_SEARCH_MS
        .filter_map(|shift_ms| residual(shift_ms).map(|residual| (shift_ms, residual)))
        .min_by(|a, b| a.1.total_cmp(&b.1))?;
    Some(ThrowDiagnosticsDto {
        track_id,
        rays: rays_per_camera(rays),
        residual_rms,
        best_time_shift_ms,
        best_residual_rms,
    })
}

/// カメラの番号ごとの視線の本数。
fn rays_per_camera(rays: &[BufferedRay]) -> BTreeMap<u32, usize> {
    let mut counts = BTreeMap::new();
    for ray in rays {
        *counts.entry(ray.camera.value()).or_default() += 1;
    }
    counts
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
    use nalgebra::Vector3;
    use struckout_proto::{CameraPose, DetectionsPacket};

    use super::*;

    struct NullEventLogger;

    impl EventLogger for NullEventLogger {
        fn push_events(&mut self, _events: TrackingEventsDto) {}

        fn push_frame(&mut self, _frame: &CameraFrame) {}
    }

    fn detection(direction: Vector3<f64>) -> Detection {
        Detection {
            bbox_width: 10,
            bbox_height: 10,
            lay_x: direction.x,
            lay_y: direction.y,
            lay_z: direction.z,
            ..Default::default()
        }
    }

    /// x = 2.5 m から的に向かって投げたボールの、`millis` ミリ秒後の位置。
    fn thrown_ball_at(millis: i64) -> Vector3<f64> {
        let t = millis as f64 / 1000.0;
        Vector3::new(2.5, 0.1, 1.4)
            + Vector3::new(-8.0, 0.5, 1.0) * t
            + Vector3::new(0.0, 0.0, -0.5 * 9.80665 * t * t)
    }

    fn time(millis: i64) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(1_790_000_000_000 + millis).unwrap()
    }

    /// 的の両脇に置いた 2 台のカメラ。
    fn side_camera_locations() -> Arc<CameraLocationStore> {
        let locations = Arc::new(CameraLocationStore::new());
        for (id, (x, y, z)) in [(0, (1.0, -0.6, 1.5)), (1, (1.2, 0.7, 1.9))] {
            let pose = CameraPose {
                x,
                y,
                z,
                ..Default::default()
            };
            locations.insert(CameraId::new(id), pose);
        }
        locations
    }

    /// カメラ `camera_id` が `millis` の時点で撮ったフレーム。
    /// `ball` が false のときは、何も映っていない。視線は端末の座標で入れる。
    fn thrown_ball_frame(
        locations: &CameraLocationStore,
        camera_id: u32,
        millis: i64,
        ball: bool,
    ) -> CameraFrame {
        let camera = locations.get(CameraId::new(camera_id)).unwrap();
        let world_direction = thrown_ball_at(millis) - camera.to_vector3();
        // このテストのカメラは z 軸まわりにしか回していないので、逆向きに回せば端末の座標になる。
        let device_direction = CameraPose {
            rotation_z_degrees: -camera.rotation_z_degrees,
            ..Default::default()
        }
        .rotate_to_world(world_direction);
        CameraFrame {
            time: time(millis),
            row_skew_ns: 0,
            received_at: time(millis),
            packet: DetectionsPacket {
                camera_id,
                session_id: format!("camera-{camera_id}"),
                timestamp: millis,
                frame_id: millis as u64,
                detections: if ball {
                    vec![detection(device_direction)]
                } else {
                    Vec::new()
                },
                ..Default::default()
            },
        }
    }

    /// 2 台がそれぞれ `millis_a`, `millis_b` の時点で撮ったフレームを、撮った順に届ける。
    fn feed(
        runner: &mut TrackRunner<NullEventLogger>,
        locations: &CameraLocationStore,
        millis_a: i64,
        millis_b: i64,
        ball: bool,
    ) -> Vec<CollisionPoint3D> {
        let mut collisions = Vec::new();
        for (camera_id, millis) in [(0, millis_a), (1, millis_b)] {
            let frame = thrown_ball_frame(locations, camera_id, millis, ball);
            collisions.extend(runner.update_frame(frame, time(millis)).0);
        }
        collisions
    }

    fn expected_collision() -> Vector3<f64> {
        thrown_ball_at(0)
            + Vector3::new(-8.0, 0.5, 1.0) * 0.3125
            + Vector3::new(0.0, 0.0, -0.5 * 9.80665 * 0.3125 * 0.3125)
    }

    #[test]
    fn reports_a_collision_predicted_before_the_ball_left_the_view() {
        let locations = side_camera_locations();
        let mut runner = TrackRunner::new(locations.clone(), NullEventLogger);

        // 約 29fps で、シャッターが 13ms ずれた 2 台に 3 フレームずつ映る。
        // 的に届くのは 312.5ms 後で、最後に映ってから 0.2 秒以上あとになる。
        for (millis_a, millis_b) in [(20, 33), (55, 68), (90, 103)] {
            assert!(feed(&mut runner, &locations, millis_a, millis_b, true).is_empty());
        }
        // 視野から出たあと、ほかの物の検出でフレームだけが届き続けても、予測は残る。
        for millis in [125, 160, 195, 230] {
            assert!(feed(&mut runner, &locations, millis, millis + 13, false).is_empty());
        }

        // 報告は、予測した着弾時刻の REPORT_LEAD (30ms) 前に出す。
        assert!(runner.poll_collisions(time(280)).0.is_empty());
        let (collisions, events) = runner.poll_collisions(time(283));

        let expected = expected_collision();
        assert_eq!(collisions.len(), 1);
        assert_relative_eq!(collisions[0].x, 0.0);
        assert_relative_eq!(collisions[0].y, expected.y, epsilon = 1e-6);
        assert_relative_eq!(collisions[0].z, expected.z, epsilon = 1e-6);
        assert!(matches!(
            events.events[0],
            TrackingEventBodyDto::Collision(_)
        ));
        // 同じ着弾を 2 回は報告しない。
        assert!(runner.poll_collisions(time(400)).0.is_empty());
    }

    #[test]
    fn predicts_a_collision_from_two_frames_per_camera() {
        let locations = side_camera_locations();
        let mut runner = TrackRunner::new(locations.clone(), NullEventLogger);

        feed(&mut runner, &locations, 20, 33, true);
        assert!(runner.tracks.is_empty());
        feed(&mut runner, &locations, 55, 68, true);

        assert_eq!(runner.tracks.len(), 1);
        assert!(runner.tracks[0].pending.is_some());
    }

    #[test]
    fn reports_a_collision_with_the_frame_that_arrives_after_it() {
        let locations = side_camera_locations();
        let mut runner = TrackRunner::new(locations.clone(), NullEventLogger);
        for (millis_a, millis_b) in [(20, 33), (55, 68)] {
            feed(&mut runner, &locations, millis_a, millis_b, true);
        }

        let (collisions, events) =
            runner.update_frame(thrown_ball_frame(&locations, 0, 330, false), time(330));

        assert_eq!(collisions.len(), 1);
        assert!(matches!(
            events.events[0],
            TrackingEventBodyDto::Collision(_)
        ));
    }

    #[test]
    fn creates_track_after_rotating_device_rays_into_world_coordinates() {
        let locations = side_camera_locations();
        locations.insert(
            CameraId::new(1),
            CameraPose {
                x: 1.2,
                y: 0.7,
                z: 1.9,
                rotation_z_degrees: 90.0,
                ..Default::default()
            },
        );
        let mut runner = TrackRunner::new(locations.clone(), NullEventLogger);

        for (millis_a, millis_b) in [(20, 33), (55, 68)] {
            feed(&mut runner, &locations, millis_a, millis_b, true);
        }

        let expected = expected_collision();
        assert_eq!(runner.tracks.len(), 1);
        let predicted = runner.tracks[0].pending.unwrap();
        assert_relative_eq!(predicted.point.y, expected.y, epsilon = 1e-6);
        assert_relative_eq!(predicted.point.z, expected.z, epsilon = 1e-6);
    }
}
