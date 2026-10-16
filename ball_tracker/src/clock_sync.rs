//! カメラが付けたフレームの時刻を、この PC の時計に直す。
//!
//! 2 台のカメラの時計は PC とも互いにも合っておらず、進み方も少し違う。着弾位置は 2 台の時刻が
//! 5ms ずれると 1cm ほど動くので、カメラごとに時計の対応を測り続ける。
//! 方式の経緯は `docs/ball_detection_plan_3_time_sync_calibration.md` の A にある。

use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use chrono::{DateTime, TimeDelta, Utc};
use parking_lot::Mutex;
use struckout_proto::{DetectionsPacket, TimestampSource, tcp_client_packet};
use tracing::{info, warn};

use crate::{CameraLocationStore, detection_input::CameraFrame, types::CameraId};

/// 時刻合わせの結果を何回ぶん覚えておくか。このうち往復時間が最も短い回を使う。
const SYNC_SAMPLES: usize = 8;
/// 対応していない送信元で、「受信時刻 − カメラの時刻」の最小値を取る期間。
/// 最小値を忘れないと、PC より遅く進む時計に追従できない。
const LEGACY_OFFSET_WINDOW: TimeDelta = TimeDelta::minutes(10);
/// 撮影から受信までの時間がこれより長くなる対応は、時計の種類の見込みが外れているとみなす。
const MAX_PLAUSIBLE_LATENCY: TimeDelta = TimeDelta::seconds(1);
/// 撮影が受信よりあとになる対応も同じ。時刻合わせの誤差のぶんだけ許す。
const MAX_PLAUSIBLE_LEAD: TimeDelta = TimeDelta::milliseconds(5);
/// 共通の遅延は、これ以上変わったときだけ更新する。更新すると全カメラの時刻が一斉に動く。
const PIPELINE_DELAY_STEP: TimeDelta = TimeDelta::milliseconds(10);
/// カメラごとの状態をログに出す間隔。
const LOG_INTERVAL: TimeDelta = TimeDelta::seconds(10);

/// 起動時に指定する補正。
#[derive(Debug, Clone, Copy, Default)]
pub struct ClockSyncOptions {
    /// 右のカメラ (登録された位置の y が最も大きいカメラ) の時刻に足す値。
    /// 時刻合わせで取りきれない一定のずれを、`estimate_time_offset.py` で求めて入れる。
    pub time_trim: TimeDelta,
    /// 画像の上端の行と下端の行の露光開始の時刻の差。指定すると、カメラが知らせた値の代わりに
    /// 全カメラでこの値を使う。
    pub rolling_shutter_skew: Option<TimeDelta>,
}

pub struct ClockSync {
    state: Mutex<State>,
    camera_locs: Arc<CameraLocationStore>,
    options: ClockSyncOptions,
    /// 右のカメラの時刻の自動の補正。`time_trim` に足す
    auto_trim: Option<Arc<AutoTimeTrim>>,
}

#[derive(Default)]
struct State {
    cameras: HashMap<u32, CameraClock>,
    legacy: HashMap<(u32, String), LegacyClock>,
    /// 時刻合わせをしたカメラのフレームの時刻に足している、共通の遅延。
    pipeline_delay: Option<TimeDelta>,
}

#[derive(Default)]
struct CameraClock {
    samples: VecDeque<SyncSample>,
    timestamp_source: TimestampSource,
    rolling_shutter_skew_ns: i64,
    /// センサー時刻を PC の時計に直せなかった。従来の方法に切り替えている。
    fallback: bool,
    /// 撮影から受信までの時間の最小値。
    min_latency: Option<TimeDelta>,
    last_log: Option<DateTime<Utc>>,
}

/// 時刻合わせ 1 回ぶんの結果。
#[derive(Debug, Clone, Copy)]
struct SyncSample {
    round_trip_ns: i64,
    /// 端末の `elapsedRealtimeNanos()` − PC の UNIX 時刻 (ns)
    offset_ns: i64,
    /// 端末の `elapsedRealtimeNanos()` − `System.nanoTime()`。端末が眠っていた時間のぶんだけ開く
    elapsed_minus_uptime_ns: i64,
}

impl CameraClock {
    fn best_sample(&self) -> Option<SyncSample> {
        self.samples
            .iter()
            .copied()
            .min_by_key(|sample| sample.round_trip_ns)
    }
}

/// 「受信時刻 − カメラの時刻」の、直近の期間での最小値。
#[derive(Default)]
struct LegacyClock {
    /// (受信時刻, ずれ)。ずれの小さい順に並ぶようにしてあり、先頭が最小値。
    offsets: VecDeque<(DateTime<Utc>, TimeDelta)>,
}

impl LegacyClock {
    fn offset(&mut self, received_at: DateTime<Utc>, observed: TimeDelta) -> TimeDelta {
        while self
            .offsets
            .front()
            .is_some_and(|(time, _)| received_at - *time > LEGACY_OFFSET_WINDOW)
        {
            self.offsets.pop_front();
        }
        while self
            .offsets
            .back()
            .is_some_and(|(_, offset)| *offset >= observed)
        {
            self.offsets.pop_back();
        }
        self.offsets.push_back((received_at, observed));
        self.offsets.front().unwrap().1
    }
}

impl ClockSync {
    pub fn new(camera_locs: Arc<CameraLocationStore>, options: ClockSyncOptions) -> Self {
        Self {
            state: Mutex::new(State::default()),
            camera_locs,
            options,
            auto_trim: None,
        }
    }

    /// 右のカメラの時刻に、[`AutoTimeTrim`] が決めた補正も足す。
    pub fn with_auto_trim(mut self, auto_trim: Arc<AutoTimeTrim>) -> Self {
        self.auto_trim = Some(auto_trim);
        self
    }

    /// 時刻合わせの往復 1 回ぶんを記録する。
    ///
    /// `sent_at` は要求を送った時刻、`received_at` は返事を受け取った時刻 (どちらもこの PC の時計)。
    pub fn record_sync(
        &self,
        camera_id: u32,
        sent_at: DateTime<Utc>,
        received_at: DateTime<Utc>,
        response: &tcp_client_packet::TimeSyncResponse,
    ) {
        let (Some(t1), Some(t4)) = (
            sent_at.timestamp_nanos_opt(),
            received_at.timestamp_nanos_opt(),
        ) else {
            return;
        };
        let t2 = response.receive_elapsed_realtime_ns;
        let t3 = response.send_elapsed_realtime_ns;
        // NTP と同じ計算。行きと帰りの遅れが同じなら、遅れは打ち消し合う。
        let offset_ns = (((t2 as i128 - t1 as i128) + (t3 as i128 - t4 as i128)) / 2) as i64;
        let round_trip_ns = (t4 - t1) - (t3 - t2);
        if round_trip_ns < 0 {
            warn!(camera_id, round_trip_ns, "ignored an implausible time sync response");
            return;
        }
        let mut state = self.state.lock();
        let camera = state.cameras.entry(camera_id).or_default();
        if camera.samples.len() >= SYNC_SAMPLES {
            camera.samples.pop_front();
        }
        camera.samples.push_back(SyncSample {
            round_trip_ns,
            offset_ns,
            elapsed_minus_uptime_ns: t3 - response.send_uptime_ns,
        });
    }

    /// カメラが知らせた設定を記録する。
    pub fn record_info(&self, camera_id: u32, info: &tcp_client_packet::CameraInfo) {
        let mut state = self.state.lock();
        let camera = state.cameras.entry(camera_id).or_default();
        camera.timestamp_source = info.timestamp_source();
        camera.rolling_shutter_skew_ns = info.rolling_shutter_skew_ns;
        // 時計の種類が分かったので、センサー時刻をもう一度試す。
        camera.fallback = false;
    }

    /// 受け取ったフレームに、この PC の時計での撮影時刻を付ける。
    ///
    /// 時刻合わせをしたカメラでは「撮影時刻 + 共通の遅延」([`Self::pipeline_delay`])、
    /// していないカメラでは「撮影時刻 + そのカメラの撮影から受信までの最短の時間」になる。
    /// どちらもフレームの時刻が受信時刻の少し前にそろうので、追跡は予測した着弾時刻を今の時刻と
    /// そのまま比べられる。時刻が壊れているフレームは `None` を返す。
    pub fn frame(&self, received_at: DateTime<Utc>, packet: DetectionsPacket) -> Option<CameraFrame> {
        let mut state = self.state.lock();
        let captured_at = Self::capture_time(&mut state, received_at, &packet);
        let time = match captured_at {
            Some(captured_at) => captured_at + Self::update_pipeline_delay(&mut state),
            None => Self::legacy_time(&mut state, received_at, &packet)?,
        };
        let camera = state.cameras.entry(packet.camera_id).or_default();
        let row_skew_ns = match self.options.rolling_shutter_skew {
            Some(skew) => skew.num_nanoseconds().unwrap_or(0),
            None => camera.rolling_shutter_skew_ns,
        };
        if camera
            .last_log
            .is_none_or(|last| received_at - last >= LOG_INTERVAL)
        {
            camera.last_log = Some(received_at);
            let best = camera.best_sample();
            info!(
                camera_id = packet.camera_id,
                synced = captured_at.is_some(),
                fallback = camera.fallback,
                timestamp_source = ?camera.timestamp_source,
                clock_offset_ms = best.map(|sample| sample.offset_ns as f64 / 1e6),
                round_trip_ms = best.map(|sample| sample.round_trip_ns as f64 / 1e6),
                min_latency_ms = camera.min_latency.map(|latency| latency.as_seconds_f64() * 1e3),
                row_skew_ms = row_skew_ns as f64 / 1e6,
                "camera clock"
            );
        }
        drop(state);

        let trim = if self.camera_locs.rightmost() == Some(CameraId::new(packet.camera_id)) {
            self.options.time_trim
                + self
                    .auto_trim
                    .as_ref()
                    .map_or(TimeDelta::zero(), |auto| auto.current(packet.camera_id))
        } else {
            TimeDelta::zero()
        };
        Some(CameraFrame {
            time: time + trim,
            row_skew_ns,
            received_at,
            packet,
        })
    }

    /// 時刻合わせをしたカメラのフレームの時刻に足している、共通の遅延。真の撮影時刻が要るときは引く。
    pub fn pipeline_delay(&self) -> TimeDelta {
        self.state.lock().pipeline_delay.unwrap_or_default()
    }

    /// センサー時刻から、この PC の時計での撮影時刻を求める。求められないときは `None`。
    fn capture_time(
        state: &mut State,
        received_at: DateTime<Utc>,
        packet: &DetectionsPacket,
    ) -> Option<DateTime<Utc>> {
        if packet.sensor_timestamp_ns == 0 {
            return None;
        }
        let camera = state.cameras.entry(packet.camera_id).or_default();
        if camera.fallback {
            return None;
        }
        let sample = camera.best_sample()?;
        // センサー時刻の時計は端末による。分からないときは両方を試す。
        let to_elapsed_ns: &[i64] = match camera.timestamp_source {
            TimestampSource::Realtime => &[0],
            TimestampSource::Unknown => &[sample.elapsed_minus_uptime_ns],
            TimestampSource::Unspecified => &[0, sample.elapsed_minus_uptime_ns],
        };
        let captured_at = to_elapsed_ns.iter().find_map(|to_elapsed_ns| {
            let unix_ns = packet.sensor_timestamp_ns + to_elapsed_ns - sample.offset_ns;
            let captured_at = DateTime::from_timestamp_nanos(unix_ns);
            let latency = received_at - captured_at;
            (-MAX_PLAUSIBLE_LEAD..=MAX_PLAUSIBLE_LATENCY)
                .contains(&latency)
                .then_some((captured_at, latency))
        });
        let Some((captured_at, latency)) = captured_at else {
            warn!(
                camera_id = packet.camera_id,
                timestamp_source = ?camera.timestamp_source,
                "sensor timestamps do not match the synced clock; falling back to arrival times"
            );
            camera.fallback = true;
            camera.min_latency = None;
            return None;
        };
        camera.min_latency = Some(
            camera
                .min_latency
                .map_or(latency, |current| current.min(latency)),
        );
        Some(captured_at)
    }

    /// 時刻合わせをしたカメラの「撮影から受信までの最短の時間」のうち、最も長いもの。
    fn update_pipeline_delay(state: &mut State) -> TimeDelta {
        let slowest = state
            .cameras
            .values()
            .filter(|camera| !camera.fallback)
            .filter_map(|camera| camera.min_latency)
            .max()
            .unwrap_or_default();
        match state.pipeline_delay {
            Some(current) if (slowest - current).abs() < PIPELINE_DELAY_STEP => current,
            _ => {
                state.pipeline_delay = Some(slowest);
                slowest
            }
        }
    }

    /// 時刻合わせをしていない送信元の時刻。「受信時刻 − カメラの時刻」の最小値を時計のずれとみなす。
    /// 撮影から受信までの遅れの最小値がカメラごとに違うと、そのぶん 2 台の時刻がずれる。
    fn legacy_time(
        state: &mut State,
        received_at: DateTime<Utc>,
        packet: &DetectionsPacket,
    ) -> Option<DateTime<Utc>> {
        let Some(camera_timestamp) = DateTime::from_timestamp_millis(packet.timestamp) else {
            warn!(
                timestamp = packet.timestamp,
                "received an invalid frame timestamp"
            );
            return None;
        };
        let offset = state
            .legacy
            .entry((packet.camera_id, packet.session_id.clone()))
            .or_default()
            .offset(received_at, received_at - camera_timestamp);
        Some(camera_timestamp + offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(camera_id: u32, timestamp: i64, frame_id: u64) -> DetectionsPacket {
        DetectionsPacket {
            camera_id,
            session_id: format!("camera-{camera_id}"),
            timestamp,
            frame_id,
            ..Default::default()
        }
    }

    fn time(timestamp: i64) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(timestamp).unwrap()
    }

    #[test]
    fn compensates_for_camera_clock_offsets() {
        let clock = ClockSync::new(
            Arc::new(CameraLocationStore::new()),
            ClockSyncOptions::default(),
        );

        let a = clock.frame(time(10_000), packet(0, 1_000, 1)).unwrap();
        let b = clock.frame(time(10_012), packet(1, 2_200, 2)).unwrap();
        // 2 フレーム目は、1 フレーム目より 5ms 余計に遅れて届いた。
        let a_late = clock.frame(time(10_038), packet(0, 1_033, 3)).unwrap();

        assert_eq!(a.time, time(10_000));
        assert_eq!(b.time, time(10_012));
        assert_eq!(a_late.time, time(10_033));
    }
}

/// 右のカメラの時刻の補正を、報告した投球から自動で決める (`--auto-time-trim`)。
///
/// 2 台の時計を合わせても数 ms のずれが残り、つなぎ直すたびに変わる (2026-10-16 は午前 -5 ms、午後 +5 ms)。
/// 追跡は報告した投球ごとに、右のカメラの時刻をずらして当てはめ直し、最もよく合うずれを出す
/// ([`ThrowDiagnosticsDto::best_time_shift_ms`])。そのずれに、そのときかけていた補正を足したものを
/// 「必要な補正」とみなし、直近の投球の中央値に向けて補正を少しずつ動かす。
///
/// - カメラの位置と向きが合っていることが前提。向きがずれていると、その誤差まで時刻で吸収しようとして、
///   着弾位置がかえってずれる (2026-10-16 の午後の録画で、向きを較正する前は、残差が最小になる補正が
///   正解に最も合う補正より 10 ms 小さかった)。
/// - 補正は、決めたときの 2 台の組み合わせ (camera_id) にだけかける。どちらかがつなぎ直すと camera_id が
///   変わるので、補正は 0 に戻り、ためた投球も捨てる。
/// - `apply` が偽なら、補正はかけずに、決めた値をログに出すだけ。
pub struct AutoTimeTrim {
    apply: bool,
    state: Mutex<AutoTrimState>,
}

#[derive(Default)]
struct AutoTrimState {
    /// (左のカメラ, 右のカメラ)。補正はこの組み合わせのときだけかける
    pair: Option<(u32, u32)>,
    /// 直近の投球の「必要な補正」(ms)
    needed: VecDeque<f64>,
    /// いまかけている補正 (ms)
    current_ms: f64,
}

/// 中央値を取る投球の数。
const AUTO_TRIM_WINDOW: usize = 9;
/// 補正を決め始めるまでに要る投球の数。
const AUTO_TRIM_MIN_THROWS: usize = 3;
/// 1 回の更新で、中央値までの差のどれだけ動かすか。外れ値が続いても急に動かないように。
const AUTO_TRIM_GAIN: f64 = 0.5;
/// 使う投球の条件: 2 台とも、この本数以上の視線がある。
const AUTO_TRIM_MIN_RAYS: usize = 5;
/// 使う投球の条件: ずらしたあとの視線と軌道の距離 (m) がこれ以下。
const AUTO_TRIM_MAX_RESIDUAL: f64 = 0.015;
/// 補正の上限 (ms)。これを超える値は、時刻ではなく別の原因 (位置や向き) とみなす。
const AUTO_TRIM_LIMIT_MS: f64 = 30.0;

impl AutoTimeTrim {
    pub fn new(apply: bool) -> Self {
        Self { apply, state: Mutex::new(AutoTrimState::default()) }
    }

    /// 右のカメラ `camera_id` のフレームの時刻に足す補正。
    pub fn current(&self, camera_id: u32) -> TimeDelta {
        let state = self.state.lock();
        match state.pair {
            Some((_, right)) if self.apply && right == camera_id => {
                TimeDelta::nanoseconds((state.current_ms * 1e6).round() as i64)
            }
            _ => TimeDelta::zero(),
        }
    }

    /// いま決まっている補正 (ms) と、それを決めた投球の数。`apply` が偽でも、決めた値を返す。
    pub fn summary(&self) -> (f64, usize) {
        let state = self.state.lock();
        (state.current_ms, state.needed.len())
    }

    /// 報告した投球の時刻合わせの手がかりを受け取る。`right` は右のカメラ (位置の y が大きいほう)。
    pub fn observe(&self, right: u32, diagnostics: &crate::tracking::ThrowDiagnosticsDto) {
        let cameras: Vec<u32> = diagnostics.rays.keys().copied().collect();
        let Some(&left) = cameras.iter().find(|&&id| id != right) else {
            return;
        };
        let enough_rays = diagnostics.rays.values().all(|&count| count >= AUTO_TRIM_MIN_RAYS) && cameras.len() == 2;
        let shift = diagnostics.best_time_shift_ms;
        let at_edge = !TIME_SHIFT_INTERIOR.contains(&shift);
        let mut state = self.state.lock();
        if state.pair != Some((left, right)) {
            if state.pair.is_some() {
                info!(left, right, "cameras changed; restarting the automatic time trim");
            }
            *state = AutoTrimState { pair: Some((left, right)), ..Default::default() };
        }
        if !enough_rays || at_edge || diagnostics.best_residual_rms > AUTO_TRIM_MAX_RESIDUAL {
            return;
        }
        // ずれは、そのときかけていた補正のうえで測っている
        let applied = if self.apply { state.current_ms } else { 0.0 };
        let needed = applied + shift as f64;
        if state.needed.len() >= AUTO_TRIM_WINDOW {
            state.needed.pop_front();
        }
        state.needed.push_back(needed);
        if state.needed.len() < AUTO_TRIM_MIN_THROWS {
            return;
        }
        let mut sorted: Vec<f64> = state.needed.iter().copied().collect();
        sorted.sort_by(f64::total_cmp);
        let median = sorted[sorted.len() / 2];
        let target = median.clamp(-AUTO_TRIM_LIMIT_MS, AUTO_TRIM_LIMIT_MS);
        if self.apply {
            state.current_ms += AUTO_TRIM_GAIN * (target - state.current_ms);
        } else {
            state.current_ms = target;
        }
        info!(
            right,
            throws = state.needed.len(),
            median_ms = format!("{median:.1}"),
            trim_ms = format!("{:.1}", state.current_ms),
            applied = self.apply,
            "automatic time trim for the right camera (added to --time-trim-ms)"
        );
    }
}

/// [`crate::tracking`] がずれを探す範囲の、端を除いた部分。端に張り付いた値は、本当のずれが範囲の外にある。
const TIME_SHIFT_INTERIOR: std::ops::RangeInclusive<i64> = -29..=59;

#[cfg(test)]
mod auto_trim_tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::tracking::ThrowDiagnosticsDto;

    fn throw(left: u32, right: u32, shift_ms: i64, rays: usize, residual: f64) -> ThrowDiagnosticsDto {
        ThrowDiagnosticsDto {
            track_id: serde_json::from_str("1").unwrap(),
            rays: BTreeMap::from([(left, rays), (right, rays)]),
            residual_rms: residual,
            best_time_shift_ms: shift_ms,
            best_residual_rms: residual,
        }
    }

    fn ms(trim: &AutoTimeTrim, camera: u32) -> f64 {
        trim.current(camera).num_microseconds().unwrap() as f64 / 1000.0
    }

    #[test]
    fn moves_toward_the_median_once_enough_throws_arrive() {
        let trim = AutoTimeTrim::new(true);
        // 本当に必要な補正は +8 ms。補正をかけるたびに、残るずれはそのぶん小さくなる
        let truth = 8.0;
        let noise = [1.0, -2.0, 0.0, 3.0, -1.0, 2.0, -3.0, 0.0, 1.0, -1.0, 0.0, 2.0];
        for (i, n) in noise.iter().enumerate() {
            let shift = (truth - ms(&trim, 1) + n).round() as i64;
            trim.observe(1, &throw(0, 1, shift, 10, 0.01));
            if i < AUTO_TRIM_MIN_THROWS - 1 {
                assert_eq!(ms(&trim, 1), 0.0, "no trim before enough throws");
            }
        }
        assert!((ms(&trim, 1) - truth).abs() < 1.5, "trim {} should approach {truth}", ms(&trim, 1));
        // 左のカメラには補正をかけない
        assert_eq!(ms(&trim, 0), 0.0);
    }

    #[test]
    fn ignores_throws_with_few_rays_a_poor_fit_or_a_shift_at_the_search_edge() {
        let trim = AutoTimeTrim::new(true);
        for _ in 0..5 {
            trim.observe(1, &throw(0, 1, 10, 2, 0.01));
            trim.observe(1, &throw(0, 1, 10, 10, 0.05));
            trim.observe(1, &throw(0, 1, 60, 10, 0.01));
            trim.observe(1, &throw(0, 1, -30, 10, 0.01));
        }
        assert_eq!(ms(&trim, 1), 0.0);
    }

    #[test]
    fn restarts_when_a_camera_reconnects() {
        let trim = AutoTimeTrim::new(true);
        for _ in 0..6 {
            trim.observe(1, &throw(0, 1, 10, 10, 0.01));
        }
        assert!(ms(&trim, 1) > 5.0);
        // 右のカメラがつなぎ直して camera_id 3 になった
        assert_eq!(ms(&trim, 3), 0.0);
        trim.observe(3, &throw(0, 3, -6, 10, 0.01));
        assert_eq!(ms(&trim, 3), 0.0, "the new pair starts from zero");
        assert_eq!(ms(&trim, 1), 0.0, "the old pair is forgotten");
    }

    #[test]
    fn only_reports_when_not_applying() {
        let trim = AutoTimeTrim::new(false);
        for _ in 0..6 {
            trim.observe(1, &throw(0, 1, 10, 10, 0.01));
        }
        assert_eq!(ms(&trim, 1), 0.0);
        assert!((trim.state.lock().current_ms - 10.0).abs() < 1e-9);
    }
}
