//! 視線の集まりからボールのトラックを作り、着弾を予測する。
//!
//! 2 台のカメラの視線が 1 本の放物線に乗り、[`RejectReason`] の条件をどれも外さないものだけをボールとみなす。
//! 手順と値の経緯は `docs/ball_detection_plan_2_track_creation.md` の 3.2〜3.5 にある。

use std::{
    collections::{BTreeMap, HashMap, hash_map::Entry},
    ops::RangeInclusive,
};

use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    tracking::{
        PredictedCollision, TrackId, TrackIdGenerator,
        ray_buffer::{BufferedRay, RayBuffer, RayLink},
        trajectory::{BallisticTrajectory, RayObservation},
    },
    types::{CameraId, CollisionPoint3D},
};

// 以下の値はすべて仮のもの。2026-10-10 の録画 (60 fps、10 球、カメラの配置は 1 通り) だけで決めている。
// 「ボールの実測」はその 10 球の値。「試作」は、計画のときに同じ処理を Python で書いて録画の検出に流したもの。
// もとのデータは `recordings/2026-10-10/notes.md`。撮影を増やしたら見直す。

// 視線とトラックの寿命

/// 視線をためておく時間。これより古い視線は捨てる。
/// 根拠: これまでの当てはめの期間と同じ長さ。ボールが 1 台に映るのは 2〜17 コマ (60 fps で 0.3 秒弱)。
pub const RAY_LIFETIME: TimeDelta = TimeDelta::milliseconds(500);

/// 着弾を予測できていないトラックは、視線が増えないままこの時間がたったら捨てる。
/// 根拠: 試作で使った値で、実測から決めたものではない。ボールはコマを飛ばさずに映っていた (60 fps で 17 ms おき)。
pub const TRACK_IDLE_TIMEOUT: TimeDelta = TimeDelta::milliseconds(150);

/// 着弾を報告したトラックを、予測した着弾時刻のあとも残しておく時間。同じボールの残りの視線を引き取らせて、
/// そこから別のトラックができるのを防ぐ。
/// 根拠: 試作で使った値で、実測から決めたものではない。
pub const REPORTED_TRACK_LINGER: TimeDelta = TimeDelta::milliseconds(150);

// 着弾の報告

/// 着弾を、予測した時刻のどれだけ前に報告するか (既定)。`--report-delay-ms` で変えられる
/// ([`super::TrackRunner::with_report_delay`])。
/// 根拠: 当てはめた着弾時刻は、着弾の音より 18〜37 ms 遅い。試作では、30 ms 早めても着弾位置の変化は最大 0.9 cm
/// だった (60 ms だと 3.8 cm)。
/// 2026-10-16 の録画では、報告を遅らせるほど着弾直前の視線を使えて精度が上がった
/// (`recordings/2026-10-16_experiment/notes.md`)。描画が着弾より遅れてもよいなら、遅らせる。
pub const REPORT_LEAD: TimeDelta = TimeDelta::milliseconds(30);

/// 着弾を報告したら、予測した着弾時刻がそこからこの時間以内のものは報告しない。同じボールからできた別のトラック、
/// 着弾のあとの跳ね返りや影を抑える。
/// 根拠: 飛んでいるボールは同時に 1 個、という前提で置いた値。録画の 10 球は 1.8 秒以上あけて投げている。
pub const REPORT_REFRACTORY: TimeDelta = TimeDelta::milliseconds(400);

// いまあるトラックを伸ばす

/// トラックに足す視線は、その時刻の軌道上の点からこの距離 (m) 以内のものに限る。
/// 根拠: 試作で使った値で、実測から決めたものではない。足したあとの当てはめを [`MAX_NOISE`] と
/// [`MAX_RAY_RESIDUAL`] で確かめるので、ここは広めにしてある。
pub const ASSOCIATION_GATE: f64 = 0.12;

// 新しいトラックを作る

/// 連結にする 2 本の視線の、時刻の差の上限。
/// 根拠: 試作で使った値で、実測から決めたものではない。録画は 60 fps (17 ms おき) で、ボールはコマを飛ばさずに
/// 映っていた。
pub const LINK_MAX_INTERVAL: TimeDelta = TimeDelta::milliseconds(40);

/// 連結にする 2 本の視線の、向きが変わる速さ (rad/s) の範囲。下限で止まっているものを、上限で離れた検出どうしを外す。
/// 根拠: ボールの実測は 1.4〜12.9 rad/s。
pub const LINK_ANGULAR_SPEED: RangeInclusive<f64> = 0.5..=60.0;

/// 1 フレームで当てはめを試す種の数の上限。ボール以外の検出が多いフレームで、処理が間に合わなくなるのを防ぐ。
/// 根拠: 試作で全検出を流すと、421 フレーム中 12 フレームがこの上限に達した (上限なしでは最大 1,815 個)。
/// 結果は上限なしのときと同じだった。
pub const MAX_SEEDS_PER_FRAME: usize = 500;

/// 種に視線を加えて当てはめ直す回数の上限。
/// 根拠: 試作で使った値で、実測から決めたものではない。
const MAX_GROWTH_ROUNDS: usize = 3;

// ボールとみなす条件

/// 1 台のカメラに要る視線の本数。2 台のそれぞれに、これだけ要る。
/// 根拠: ボールの実測で最も少ないのは 8 球目の 2+2 本 (4 球目は 4+2 本、7 球目は 5+2 本)。
pub const MIN_RAYS_PER_CAMERA: usize = 2;

/// 視線 1 本の誤差の見積もり σ (m) の上限。σ は [`BallisticTrajectory::noise`] で求める。
/// 根拠: ボールの実測は 0.8〜1.8 cm で、8 球目だけ 1.6〜2.3 cm。ボール以外の視線の寄せ集めは 2.5 cm 以上だった。
/// 余裕は小さい。試作では、2.2 cm にすると 8 球目を落とし、3.0 cm にすると余分な候補が 1 件出た。
pub const MAX_NOISE: f64 = 0.025;

/// 視線 1 本と軌道の距離 (m) の上限。種に加える視線も、この距離までのものにする。
/// 根拠: ボールの実測は最大 3.8 cm。2 台の時計が 10 ms ずれると 4.9 cm になる。
pub const MAX_RAY_RESIDUAL: f64 = 0.06;

/// 的に向かう速さ (m/s) の下限。跳ね返ったものと、横切るものを外す。
/// 根拠: ボールの実測は 2.5〜6.0 m/s。
pub const MIN_APPROACH_SPEED: f64 = 1.5;

/// 速さ (m/s) の上限。桁外れの解を外す。
/// 根拠: ボールの実測は最大 6.3 m/s。
pub const MAX_SPEED: f64 = 25.0;

/// 横 (y) の速さは、的に向かう速さのこの倍まで。横切るものを外す。
/// 根拠: ボールの実測は 0.48 倍まで。
pub const MAX_LATERAL_SPEED_RATIO: f64 = 1.0;

/// カメラから軌道上の点までの、視線に沿った距離 (m) の範囲。カメラを通り抜ける解を外す。
/// 根拠: ボールの実測は 0.34〜1.06 m。
pub const RAY_DEPTH: RangeInclusive<f64> = 0.2..=3.0;

/// 観測した時刻のボールの位置の、y (m) の範囲。筐体の内側に 5〜7 cm の余裕を足してある。
/// 根拠: 筐体データの寸法で、奥面は y 0.0455〜1.1545 m。実機の寸法と合っているかは確かめていない。
pub const BOOTH_Y: RangeInclusive<f64> = -0.02..=1.22;

/// 観測した時刻のボールの位置の、z (m) の範囲。筐体の内側に 5〜7 cm の余裕を足してある。
/// 根拠: 筐体データの寸法で、床との境目が z 1.1155 m、天井が z 2.50 m。箱に落ちた影だけの組は z = 2.61 m で、
/// 天井の上になった。
pub const BOOTH_Z: RangeInclusive<f64> = 1.05..=2.55;

/// 着弾点の y (m) の範囲。奥面をボールの半径 3.5 cm ぶん狭めて、5 cm の余裕を足してある。
/// 根拠: 奥面の寸法は [`BOOTH_Y`] と同じ。ボールの実測は y 0.47〜1.00 m。
pub const TARGET_Y: RangeInclusive<f64> = 0.03..=1.17;

/// 着弾点の z (m) の範囲。奥面をボールの半径 3.5 cm ぶん狭めて、5 cm の余裕を足してある。
/// 根拠: 奥面の寸法は [`BOOTH_Z`] と同じ。ボールの実測は z 1.51〜2.00 m。影だけの組は z = 2.56 m だった。
pub const TARGET_Z: RangeInclusive<f64> = 1.10..=2.52;

/// 最後の観測から着弾までの時間の上限。これより先の予測は当てにしない。
/// 根拠: ボールの実測は +4〜+100 ms。
pub const MAX_PREDICTION_HORIZON: TimeDelta = TimeDelta::milliseconds(600);

/// 最後の観測の時点ですでに的を通りすぎていた、という予測を受け入れる限度。
/// 根拠: これまでの値のまま。ボールの実測では、着弾は最後の観測より 4 ms 以上あとだった。
pub const MAX_PREDICTION_LATENESS: TimeDelta = TimeDelta::milliseconds(100);

/// 2 台のカメラの観測の、時間の空きの上限。種にする 2 つの連結の空きも、この値までにする。
/// 根拠: ボールの実測は、重なるか、空いても 10 ms。影 (左) と跳ね返り (右) の組は 143〜160 ms 空いていた。
pub const MAX_CAMERA_GAP: TimeDelta = TimeDelta::milliseconds(100);

/// あるカメラの最初と最後の観測の間に届いたフレームのうち、使わなかったものの数の上限。そのカメラで使った視線の
/// 本数に対する割合で、端数は切り捨てる (3 本以下なら 0)。途中のコマを飛ばした寄せ集めを外す。
/// 根拠: ボールの実測は 0。ボールはコマを飛ばさずに映っていた。
pub const MAX_SKIPPED_FRAME_RATIO: f64 = 0.25;

/// 視線の集まりをボールとみなさなかった理由。確かめる順に並べてあり、最初に外れた条件を返す。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RejectReason {
    /// カメラが 1 台だけ、または視線が [`MIN_RAYS_PER_CAMERA`] 本に満たないカメラがある
    TooFewRays,
    /// σ が [`MAX_NOISE`] を超える
    NoisyFit,
    /// 軌道から [`MAX_RAY_RESIDUAL`] より離れた視線がある
    StrayRay,
    /// 的に向かう速さが [`MIN_APPROACH_SPEED`] に満たない
    SlowApproach,
    /// 速さが [`MAX_SPEED`] を超える
    TooFast,
    /// 横の速さが、的に向かう速さの [`MAX_LATERAL_SPEED_RATIO`] 倍を超える
    Sideways,
    /// カメラからの距離が [`RAY_DEPTH`] の外にある視線がある
    DepthOutOfRange,
    /// 観測した時刻の位置が [`BOOTH_Y`]、[`BOOTH_Z`] の外にある
    OutsideBooth,
    /// 途中のフレームを [`MAX_SKIPPED_FRAME_RATIO`] より多く飛ばしている
    SkippedFrames,
    /// 2 台の観測の時間が [`MAX_CAMERA_GAP`] より空いている
    CameraGap,
    /// 着弾が、最後の観測の [`MAX_PREDICTION_HORIZON`] より先か、[`MAX_PREDICTION_LATENESS`] より前
    ImpactTime,
    /// 着弾点が [`TARGET_Y`]、[`TARGET_Z`] の外にある
    OutsideTarget,
}

/// 視線の集まりに当てはめた軌道と、その当てはまりぐあい。
#[derive(Debug, Clone, Copy)]
struct Fit {
    trajectory: BallisticTrajectory,
    /// 視線 1 本の誤差の見積もり σ (m)
    noise: f64,
    /// 視線と軌道の距離のうち、最も大きいもの (m)
    max_residual: f64,
}

impl Fit {
    /// 解が定まらないときは `None`。
    fn new(rays: &[BufferedRay]) -> Option<Self> {
        let observations: Vec<RayObservation> = rays.iter().map(|ray| ray.observation).collect();
        let trajectory = BallisticTrajectory::fit(&observations)?;
        let noise = trajectory.noise(&observations)?;
        let max_residual = trajectory.max_residual(&observations)?;
        (noise.is_finite() && max_residual.is_finite()).then_some(Self {
            trajectory,
            noise,
            max_residual,
        })
    }

    /// 残差の条件 (σ と、視線 1 本ごとの距離) を満たすか。
    fn has_small_residuals(&self) -> bool {
        self.noise <= MAX_NOISE && self.max_residual <= MAX_RAY_RESIDUAL
    }
}

/// 1 個のボールとみなした視線の集まりと、それに当てはめた軌道。[`seed_tracks`] が作る。
#[derive(Debug)]
pub struct BallTrack {
    id: TrackId,
    /// 時刻の順に並べてある。1 フレームにつき 1 本まで
    rays: Vec<BufferedRay>,
    fit: Fit,
}

impl BallTrack {
    pub fn id(&self) -> TrackId {
        self.id
    }

    /// このトラックに入っている視線。時刻の順。
    pub fn rays(&self) -> &[BufferedRay] {
        &self.rays
    }

    /// 視線 1 本の誤差の見積もり σ (m)。
    pub fn noise(&self) -> f64 {
        self.fit.noise
    }

    /// `other` よりもボールらしいか。同じボールから 2 本のトラックができたときに、残すほうを決める。
    pub fn outranks(&self, other: &BallTrack) -> bool {
        outranks(
            (self.rays.len(), self.fit.noise),
            (other.rays.len(), other.fit.noise),
        )
    }

    /// この視線をこのトラックに足す候補にできるなら、視線と、その時刻の軌道上の点との距離 (m) を返す。
    ///
    /// 次の視線は候補にしない。
    ///
    /// - 距離が [`ASSOCIATION_GATE`] を超える、または軌道がカメラの後ろ側を通る
    /// - 軌道が的の面を通る時刻よりあとのもの。跳ね返ったボールを同じトラックに入れると、当てはめが崩れる
    /// - すでに視線を持っているフレームのもの
    pub fn association_distance(&self, ray: &BufferedRay) -> Option<f64> {
        let trajectory = &self.fit.trajectory;
        if trajectory
            .plane_crossing()
            .is_some_and(|crossing| ray.time() > crossing.time)
        {
            return None;
        }
        if self
            .rays
            .iter()
            .any(|own| own.camera == ray.camera && own.time() == ray.time())
        {
            return None;
        }
        let offset = trajectory.ray_offset(&ray.observation)?;
        (offset.distance <= ASSOCIATION_GATE && offset.depth > 0.0).then_some(offset.distance)
    }

    /// 視線を足して、軌道を当てはめ直す。足したら `true` を返す。
    ///
    /// [`Self::association_distance`] が `None` を返す視線と、足すと残差の条件 ([`MAX_NOISE`]、
    /// [`MAX_RAY_RESIDUAL`]) を満たさなくなる視線は足さない。そのときトラックは変わらない。
    pub fn try_extend(&mut self, ray: &BufferedRay) -> bool {
        if self.association_distance(ray).is_none() {
            return false;
        }
        let mut rays = self.rays.clone();
        let index = rays.partition_point(|own| own.time() <= ray.time());
        rays.insert(index, *ray);
        let Some(fit) = Fit::new(&rays).filter(Fit::has_small_residuals) else {
            return false;
        };
        self.rays = rays;
        self.fit = fit;
        true
    }

    /// 的に当たると予測している時刻と位置。ボールとみなす条件を満たしていないときは、その理由を返す。
    ///
    /// `buffer` は、途中で飛ばしたフレームを数えるのに使う。フレームはあとから届くこともあるので、視線が
    /// 増えていなくても結果は変わりうる。
    pub fn predicted_collision(
        &self,
        buffer: &RayBuffer,
    ) -> Result<PredictedCollision, RejectReason> {
        predict_collision(&self.fit, &self.rays, buffer)
    }
}

/// 視線の多いほうを、同数なら σ の小さいほうを、よりボールらしいとみなす。引数は (視線の本数, σ)。
fn outranks(candidate: (usize, f64), other: (usize, f64)) -> bool {
    candidate.0 > other.0 || (candidate.0 == other.0 && candidate.1 < other.1)
}

/// 視線の集まりのうち、1 台のカメラのぶんの時刻。
struct CameraSpan {
    camera: CameraId,
    times: Vec<DateTime<Utc>>,
    first: DateTime<Utc>,
    last: DateTime<Utc>,
}

impl CameraSpan {
    /// 視線をカメラごとに分ける。`HashMap` を使わないのは、確かめる順を毎回同じにするため。
    fn split(rays: &[BufferedRay]) -> Vec<Self> {
        let mut spans: Vec<Self> = Vec::new();
        for ray in rays {
            let time = ray.time();
            match spans.iter_mut().find(|span| span.camera == ray.camera) {
                Some(span) => {
                    span.times.push(time);
                    span.first = span.first.min(time);
                    span.last = span.last.max(time);
                }
                None => spans.push(Self {
                    camera: ray.camera,
                    times: vec![time],
                    first: time,
                    last: time,
                }),
            }
        }
        spans
    }
}

/// 2 つの時間帯の空き。重なっていれば負になる。引数は (始まり, 終わり)。
fn gap_between(a: (DateTime<Utc>, DateTime<Utc>), b: (DateTime<Utc>, DateTime<Utc>)) -> TimeDelta {
    a.0.max(b.0) - a.1.min(b.1)
}

/// ボールとみなす条件を順に確かめ、すべて満たせば着弾の予測を返す。
fn predict_collision(
    fit: &Fit,
    rays: &[BufferedRay],
    buffer: &RayBuffer,
) -> Result<PredictedCollision, RejectReason> {
    let cameras = CameraSpan::split(rays);
    if cameras.len() < 2
        || cameras
            .iter()
            .any(|camera| camera.times.len() < MIN_RAYS_PER_CAMERA)
    {
        return Err(RejectReason::TooFewRays);
    }
    if fit.noise > MAX_NOISE {
        return Err(RejectReason::NoisyFit);
    }
    if fit.max_residual > MAX_RAY_RESIDUAL {
        return Err(RejectReason::StrayRay);
    }

    let trajectory = &fit.trajectory;
    let velocity = trajectory.velocity();
    if trajectory.approach_speed() < MIN_APPROACH_SPEED {
        return Err(RejectReason::SlowApproach);
    }
    if velocity.norm() > MAX_SPEED {
        return Err(RejectReason::TooFast);
    }
    if velocity.y.abs() > MAX_LATERAL_SPEED_RATIO * velocity.x.abs() {
        return Err(RejectReason::Sideways);
    }

    for ray in rays {
        let depth = trajectory
            .ray_offset(&ray.observation)
            .map(|offset| offset.depth);
        if !depth.is_some_and(|depth| RAY_DEPTH.contains(&depth)) {
            return Err(RejectReason::DepthOutOfRange);
        }
        let position = trajectory.position_at(ray.time());
        if !BOOTH_Y.contains(&position.y) || !BOOTH_Z.contains(&position.z) {
            return Err(RejectReason::OutsideBooth);
        }
    }

    for camera in &cameras {
        // バッファがもう捨てた時刻のフレームは数に入らない。古い部分は確かめずに通すことになる。
        let skipped = buffer
            .frames_between(camera.camera, camera.first, camera.last)
            .filter(|time| !camera.times.contains(time))
            .count();
        if skipped > (MAX_SKIPPED_FRAME_RATIO * camera.times.len() as f64) as usize {
            return Err(RejectReason::SkippedFrames);
        }
    }

    // どのカメラの観測とも重なる時間帯。なければ、始まりが終わりよりあとになる。
    let mut common = (cameras[0].first, cameras[0].last);
    let mut last_seen = cameras[0].last;
    for camera in &cameras[1..] {
        common = (common.0.max(camera.first), common.1.min(camera.last));
        last_seen = last_seen.max(camera.last);
    }
    if common.0 - common.1 > MAX_CAMERA_GAP {
        return Err(RejectReason::CameraGap);
    }

    let crossing = trajectory
        .plane_crossing()
        .ok_or(RejectReason::SlowApproach)?;
    let until_impact = crossing.time - last_seen;
    if until_impact > MAX_PREDICTION_HORIZON || until_impact < -MAX_PREDICTION_LATENESS {
        return Err(RejectReason::ImpactTime);
    }
    if !TARGET_Y.contains(&crossing.y) || !TARGET_Z.contains(&crossing.z) {
        return Err(RejectReason::OutsideTarget);
    }
    Ok(PredictedCollision {
        time: crossing.time,
        point: CollisionPoint3D {
            x: 0.,
            y: crossing.y,
            z: crossing.z,
        },
    })
}

/// [`seed_tracks`] が 1 フレームでしたことの集計。しきい値を見直すときの手がかりにする。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SeedDiagnostics {
    /// どのトラックにも入っていなかった視線の数
    pub free_rays: usize,
    /// このフレームの視線を含む連結の数
    pub new_links: usize,
    /// ほかのカメラの連結の数
    pub other_links: usize,
    /// 当てはめを試した種の数
    pub tested_seeds: usize,
    /// [`MAX_SEEDS_PER_FRAME`] に達して、試さなかった種がある
    pub seed_limit_reached: bool,
    /// 4 本の視線から軌道が定まらなかった種の数
    pub unsolvable_seeds: usize,
    /// 条件を満たさなかった種の数 (理由ごと)。視線を加える前に外れたものも、加えたあとで外れたものも数える
    pub rejections: BTreeMap<RejectReason, usize>,
    /// 条件をすべて満たした種の数。同じボールの種は同じ視線の集まりに育つので、ふつうトラックの数より多い
    pub passed_seeds: usize,
    /// 作ったトラックの数
    pub accepted_tracks: usize,
}

/// 種を育ててできた、トラックの候補。
struct Candidate {
    rays: Vec<BufferedRay>,
    fit: Fit,
}

/// どのトラックにも入っていない視線から、新しいトラックを作る。
///
/// `new_rays` には、いま届いたフレームの視線 ([`RayBuffer::push_frame`] が返したもの) を渡す。
/// 「このフレームの視線を含む連結」と「ほかのカメラの連結」の組を種にして 4 本に軌道を当てはめ、
/// 軌道に乗る空いた視線を加える。条件をすべて満たした候補のうち最もボールらしいものをトラックにして、
/// 空いた視線から作れなくなるまで繰り返す。トラックにした視線の持ち主は、ここで付け替える。
pub fn seed_tracks(
    buffer: &mut RayBuffer,
    new_rays: &[BufferedRay],
    id_gen: &TrackIdGenerator,
) -> (Vec<BallTrack>, SeedDiagnostics) {
    let mut diagnostics = SeedDiagnostics::default();
    let mut tracks = Vec::new();
    // トラックを 1 本作ると空いている視線が減るので、そのたびに候補を作り直す。
    while let Some(candidate) = best_candidate(buffer, new_rays, &mut diagnostics) {
        let id = id_gen.next();
        for ray in &candidate.rays {
            buffer.set_owner(ray.id, Some(id));
        }
        tracks.push(BallTrack {
            id,
            rays: candidate.rays,
            fit: candidate.fit,
        });
    }
    diagnostics.accepted_tracks = tracks.len();
    (tracks, diagnostics)
}

/// 空いている視線から作れる候補のうち、最もボールらしいもの。
fn best_candidate(
    buffer: &RayBuffer,
    new_rays: &[BufferedRay],
    diagnostics: &mut SeedDiagnostics,
) -> Option<Candidate> {
    let camera = new_rays.first()?.camera;
    let free: Vec<BufferedRay> = buffer.free_rays().copied().collect();
    let is_new = |ray: &BufferedRay| new_rays.iter().any(|new| new.id == ray.id);
    let mut new_links = Vec::new();
    let mut other_links = Vec::new();
    for link in buffer.free_links() {
        if link.earlier.camera != camera {
            other_links.push(link);
        } else if is_new(&link.earlier) || is_new(&link.later) {
            new_links.push(link);
        }
    }
    // トラックを作ったあとの繰り返しでは上書きしない (種を 1 つも試さなかったときは、繰り返しがない)。
    if diagnostics.tested_seeds == 0 {
        diagnostics.free_rays = free.len();
        diagnostics.new_links = new_links.len();
        diagnostics.other_links = other_links.len();
    }

    // 種は、2 つの連結の時間の空きが小さいものから試す。上限に達したら、空きの大きいものを試さずに終える。
    let new_groups = group_by_span(new_links);
    let other_groups = group_by_span(other_links);
    let mut group_pairs = Vec::new();
    for (new_span, new_group) in &new_groups {
        for (other_span, other_group) in &other_groups {
            let gap = gap_between(*new_span, *other_span);
            if gap <= MAX_CAMERA_GAP {
                group_pairs.push((gap, new_group, other_group));
            }
        }
    }
    group_pairs.sort_by_key(|(gap, _, _)| *gap);

    let mut best: Option<Candidate> = None;
    'seeds: for (_, new_group, other_group) in group_pairs {
        for new_link in new_group {
            for other_link in other_group {
                if diagnostics.tested_seeds >= MAX_SEEDS_PER_FRAME {
                    diagnostics.seed_limit_reached = true;
                    break 'seeds;
                }
                diagnostics.tested_seeds += 1;
                let Some(candidate) = grow_seed(new_link, other_link, &free, buffer, diagnostics)
                else {
                    continue;
                };
                let is_better = best.as_ref().is_none_or(|best| {
                    outranks(
                        (candidate.rays.len(), candidate.fit.noise),
                        (best.rays.len(), best.fit.noise),
                    )
                });
                if is_better {
                    best = Some(candidate);
                }
            }
        }
    }
    best
}

/// 連結を、両端の時刻が同じものごとにまとめる。
///
/// 同じまとまりの連結は、相手の連結との時間の空きも同じになる。まとまりの数は届いたフレームの数で決まり、
/// 検出の数にはよらないので、種を空きの小さい順に並べるのに、種をすべて並べなくて済む。
fn group_by_span(links: Vec<RayLink>) -> BTreeMap<(DateTime<Utc>, DateTime<Utc>), Vec<RayLink>> {
    let mut groups: BTreeMap<_, Vec<RayLink>> = BTreeMap::new();
    for link in links {
        let span = (link.earlier.time(), link.later.time());
        groups.entry(span).or_default().push(link);
    }
    groups
}

/// 種 (2 つの連結の 4 本) に軌道を当てはめ、軌道に乗る空いた視線を加える。
/// ボールとみなす条件を満たさなければ、理由を `diagnostics` に数えて `None` を返す。
fn grow_seed(
    new_link: &RayLink,
    other_link: &RayLink,
    free: &[BufferedRay],
    buffer: &RayBuffer,
    diagnostics: &mut SeedDiagnostics,
) -> Option<Candidate> {
    let mut rays = vec![
        new_link.earlier,
        new_link.later,
        other_link.earlier,
        other_link.later,
    ];
    rays.sort_by_key(|ray| (ray.time(), ray.id));
    let Some(mut fit) = Fit::new(&rays) else {
        diagnostics.unsolvable_seeds += 1;
        return None;
    };
    // 視線を加える前に、4 本だけでわかる条件 (残差と速さ) で絞る。4 本では、視線 1 本ごとの距離は σ の
    // √2 倍を超えないので、σ だけ確かめれば足りる。
    let seed_check = if fit.noise > MAX_NOISE {
        Err(RejectReason::NoisyFit)
    } else if fit.trajectory.approach_speed() < MIN_APPROACH_SPEED {
        Err(RejectReason::SlowApproach)
    } else if fit.trajectory.velocity().norm() > MAX_SPEED {
        Err(RejectReason::TooFast)
    } else {
        Ok(())
    };
    if let Err(reason) = seed_check {
        *diagnostics.rejections.entry(reason).or_default() += 1;
        return None;
    }

    // 軌道に乗る視線を選び直しては当てはめ直す。選ぶ視線が変わらなくなったら終わり。
    // 選び直すので、種の視線が外れることもある。
    for _ in 0..MAX_GROWTH_ROUNDS {
        let grown = rays_on_trajectory(&fit.trajectory, free);
        let Some(refit) = Fit::new(&grown) else {
            break;
        };
        let unchanged = grown
            .iter()
            .map(|ray| ray.id)
            .eq(rays.iter().map(|ray| ray.id));
        rays = grown;
        fit = refit;
        if unchanged {
            break;
        }
    }

    match predict_collision(&fit, &rays, buffer) {
        Ok(_) => {
            diagnostics.passed_seeds += 1;
            Some(Candidate { rays, fit })
        }
        Err(reason) => {
            *diagnostics.rejections.entry(reason).or_default() += 1;
            None
        }
    }
}

/// 空いている視線のうち、軌道から [`MAX_RAY_RESIDUAL`] 以内にあるもの。同じフレームに 2 本以上あれば、
/// 最も近い 1 本だけを選ぶ。時刻の順に並べて返す。
fn rays_on_trajectory(trajectory: &BallisticTrajectory, free: &[BufferedRay]) -> Vec<BufferedRay> {
    let mut nearest: HashMap<(CameraId, DateTime<Utc>), (f64, BufferedRay)> = HashMap::new();
    for ray in free {
        let Some(offset) = trajectory.ray_offset(&ray.observation) else {
            continue;
        };
        if offset.distance > MAX_RAY_RESIDUAL || offset.depth <= 0.0 {
            continue;
        }
        match nearest.entry((ray.camera, ray.time())) {
            Entry::Occupied(mut entry) => {
                if offset.distance < entry.get().0 {
                    entry.insert((offset.distance, *ray));
                }
            }
            Entry::Vacant(entry) => {
                entry.insert((offset.distance, *ray));
            }
        }
    }
    let mut rays: Vec<BufferedRay> = nearest.into_values().map(|(_, ray)| ray).collect();
    rays.sort_by_key(|ray| (ray.time(), ray.id));
    rays
}
