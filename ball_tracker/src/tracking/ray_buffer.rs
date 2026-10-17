//! 直近の視線をためておくバッファ。

use std::collections::{BTreeSet, HashMap};

use chrono::{DateTime, TimeDelta, Utc};
use nalgebra::Vector3;

use crate::{
    tracking::{
        TrackId,
        ball_track::{LINK_ANGULAR_SPEED, LINK_MAX_INTERVAL},
        trajectory::RayObservation,
    },
    types::CameraId,
};

/// バッファの中で視線を区別する番号。入れた順に大きくなる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RayId(u64);

/// バッファに入れた視線。
#[derive(Debug, Clone, Copy)]
pub struct BufferedRay {
    pub id: RayId,
    pub camera: CameraId,
    /// この視線が入っていたフレームの時刻
    frame_time: DateTime<Utc>,
    /// 時刻は、この検出が撮影された時刻。行ごとの露光の時刻差があると、フレームの時刻より少し遅い。
    /// 向きは長さ 1 にそろえてある
    pub observation: RayObservation,
}

impl BufferedRay {
    /// この視線が入っていたフレームの時刻。同じフレームの視線かどうかは、これで見分ける。
    /// 軌道の当てはめには `observation` の時刻を使う。
    pub fn time(&self) -> DateTime<Utc> {
        self.frame_time
    }
}

/// 同じカメラの 2 本の視線の「連結」。同じものが動いて、続けて映ったとみなせる組。
#[derive(Debug, Clone, Copy)]
pub struct RayLink {
    /// 時刻の早いほう
    pub earlier: BufferedRay,
    pub later: BufferedRay,
}

struct Slot {
    ray: BufferedRay,
    /// この視線を使っているトラック。どのトラックにも入っていなければ `None`
    owner: Option<TrackId>,
}

/// 直近の視線と、それぞれの持ち主 (トラック)、カメラごとの届いたフレームの時刻を持つ。
///
/// 2 台のフレームを組にしないで、1 台ぶんずつここにためる。片方のカメラにしか映っていない時間の視線も、
/// あとから届いたもう一方の視線と合わせて使える。
pub struct RayBuffer {
    /// 入れた順。番号の小さい順でもある
    slots: Vec<Slot>,
    /// カメラごとの、届いたフレームの時刻。検出が 1 つもなかったフレームも入る
    frame_times: HashMap<CameraId, BTreeSet<DateTime<Utc>>>,
    /// 最後の [`Self::prune`] で決めた、残す時刻の下限
    oldest: Option<DateTime<Utc>>,
    next_id: u64,
}

impl RayBuffer {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            frame_times: HashMap::new(),
            oldest: None,
            next_id: 0,
        }
    }

    /// 1 台のカメラの 1 フレームぶんの視線を入れ、入れた視線を返す。入れた視線は空いている (持ち主がない)。
    ///
    /// `time` はフレームの時刻。検出が 1 つもないフレームでも、届いたことは覚えておく
    /// ([`Self::frames_between`] が数える)。次のものは入れない。
    ///
    /// - [`Self::prune`] で捨てた時刻より前のフレーム。フレームごと入れず、空の `Vec` を返す
    /// - 向きの決まらない視線 (長さが 0、成分が有限でない) と、カメラの位置が有限でない視線
    pub fn push_frame(
        &mut self,
        camera: CameraId,
        time: DateTime<Utc>,
        rays: impl IntoIterator<Item = RayObservation>,
    ) -> Vec<BufferedRay> {
        if self.oldest.is_some_and(|oldest| time < oldest) {
            return Vec::new();
        }
        self.frame_times.entry(camera).or_default().insert(time);

        let mut pushed = Vec::new();
        for ray in rays {
            let Some(direction) = ray.direction.try_normalize(f64::EPSILON) else {
                continue;
            };
            if !is_finite(&direction) || !is_finite(&ray.origin) {
                continue;
            }
            let ray = BufferedRay {
                id: RayId(self.next_id),
                camera,
                frame_time: time,
                observation: RayObservation {
                    time: ray.time,
                    origin: ray.origin,
                    direction,
                },
            };
            self.next_id += 1;
            self.slots.push(Slot { ray, owner: None });
            pushed.push(ray);
        }
        pushed
    }

    /// `oldest` より前の視線とフレームの時刻を捨てる。持ち主のある視線も捨てる。
    pub fn prune(&mut self, oldest: DateTime<Utc>) {
        self.slots.retain(|slot| slot.ray.time() >= oldest);
        self.frame_times.retain(|_, times| {
            *times = times.split_off(&oldest);
            !times.is_empty()
        });
        self.oldest = Some(oldest);
    }

    /// 視線の持ち主を付け替える。`None` を渡すと空きに戻る。もう捨てた視線なら何もしない。
    pub fn set_owner(&mut self, id: RayId, owner: Option<TrackId>) {
        if let Ok(index) = self.slots.binary_search_by_key(&id, |slot| slot.ray.id) {
            self.slots[index].owner = owner;
        }
    }

    /// どのトラックにも入っていない視線。入れた順に返す。
    pub fn free_rays(&self) -> impl Iterator<Item = &BufferedRay> {
        self.slots
            .iter()
            .filter(|slot| slot.owner.is_none())
            .map(|slot| &slot.ray)
    }

    /// `camera` から届いたフレームのうち、時刻が `from` から `to` まで (両端を含む) のものの時刻。
    ///
    /// [`Self::prune`] で捨てた時刻より前のフレームは、届いていても数に入らない。
    pub fn frames_between(
        &self,
        camera: CameraId,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> impl Iterator<Item = DateTime<Utc>> {
        // `BTreeSet::range` は、始まりが終わりよりあとだと panic する。
        let times = self.frame_times.get(&camera).filter(|_| from <= to);
        times
            .into_iter()
            .flat_map(move |times| times.range(from..=to).copied())
    }

    /// 空いている視線どうしの連結を、すべて挙げる。
    ///
    /// 連結にするのは、同じカメラの 2 本で、時刻の差が [`LINK_MAX_INTERVAL`] 以内、視線の向きが変わる速さが
    /// [`LINK_ANGULAR_SPEED`] の範囲にあるもの。止まっているものと、離れた検出どうしは連結にならない。
    pub fn free_links(&self) -> Vec<RayLink> {
        // カメラごとに分ける。`HashMap` を使わないのは、挙げる順を毎回同じにするため。
        let mut cameras: Vec<(CameraId, Vec<BufferedRay>)> = Vec::new();
        for ray in self.free_rays() {
            match cameras.iter_mut().find(|(camera, _)| *camera == ray.camera) {
                Some((_, rays)) => rays.push(*ray),
                None => cameras.push((ray.camera, vec![*ray])),
            }
        }

        let mut links = Vec::new();
        for (_, rays) in &mut cameras {
            // フレームは時刻の順に届くとは限らない。
            rays.sort_by_key(|ray| ray.time());
            for (index, earlier) in rays.iter().enumerate() {
                for later in &rays[index + 1..] {
                    let interval = later.time() - earlier.time();
                    if interval > LINK_MAX_INTERVAL {
                        break;
                    }
                    if interval <= TimeDelta::zero() {
                        // 同じフレームの視線
                        continue;
                    }
                    let angle = earlier
                        .observation
                        .direction
                        .angle(&later.observation.direction);
                    if LINK_ANGULAR_SPEED.contains(&(angle / interval.as_seconds_f64())) {
                        links.push(RayLink {
                            earlier: *earlier,
                            later: *later,
                        });
                    }
                }
            }
        }
        links
    }
}

fn is_finite(vector: &Vector3<f64>) -> bool {
    vector.iter().all(|component| component.is_finite())
}
