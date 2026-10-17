//! 放物線の当てはめと、的の面との交点の計算。

use chrono::{DateTime, TimeDelta, Utc};
use nalgebra::{Matrix3, Matrix6, Vector3, Vector6};

pub const GRAVITY_ACCELERATION: f64 = 9.80665;

/// これより少ない視線からは当てはめない。未知数は 6 個で視線 1 本が式 2 本ぶんなので
/// 3 本あれば解けるが、余りがないと外れた視線に気づけない。
const MIN_RAYS: usize = 4;
/// 正規方程式の最小の特異値が最大の何倍より小さければ、解が定まっていないとみなすか。
/// 同じ時刻の視線しかなく、速度が決まらないときにこうなる。
const MIN_RECIPROCAL_CONDITION: f64 = 1e-9;

/// あるカメラが、ある時刻にボールを見た方向。
#[derive(Debug, Clone, Copy)]
pub struct RayObservation {
    pub time: DateTime<Utc>,
    /// カメラの位置 (世界座標)
    pub origin: Vector3<f64>,
    /// ボールが見えた向き (世界座標)。長さは問わない
    pub direction: Vector3<f64>,
}

/// 重力だけを受けて飛ぶボールの軌道。
#[derive(Debug, Clone, Copy)]
pub struct BallisticTrajectory {
    epoch: DateTime<Utc>,
    position: Vector3<f64>,
    velocity: Vector3<f64>,
    /// 当てはめに使った視線と軌道との距離の二乗平均平方根
    pub residual_rms: f64,
}

/// 軌道が的の面 (x = 0) を通る時刻と位置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaneCrossing {
    pub time: DateTime<Utc>,
    pub y: f64,
    pub z: f64,
}

/// 視線と軌道の離れぐあい。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayOffset {
    /// 視線を延ばした直線と、視線の時刻の軌道上の点との距離 (m)
    pub distance: f64,
    /// カメラからその点までの、視線に沿った距離 (m)。カメラの後ろ側にあれば負になる
    pub depth: f64,
}

impl BallisticTrajectory {
    /// 視線の集まりに、最小二乗法で軌道を当てはめる。
    ///
    /// 三角測量した点ではなく視線そのものを使う。2 台のシャッターは同期しておらず、
    /// 組にしたフレームでも撮影時刻が最大 20ms ずれるので、同時刻とみなして三角測量すると
    /// 速い球では 10cm 以上ずれる。視線ごとに「その時刻の軌道上の点との距離」を見れば、
    /// ずれた時刻のまま使える。この距離は未知数 (初期位置と初速) の一次式なので、反復なしで解ける。
    ///
    /// 解が定まらないときは `None` を返す。
    pub fn fit(observations: &[RayObservation]) -> Option<Self> {
        if observations.len() < MIN_RAYS {
            return None;
        }
        // 桁落ちを避けるため、時刻は最後の観測からの秒数で扱う。
        let epoch = observations.iter().map(|o| o.time).max()?;

        // 正規方程式を、位置と速度の 3x3 のブロックごとに足し込む。
        let mut sum_p = Matrix3::<f64>::zeros();
        let mut sum_p_tau = Matrix3::<f64>::zeros();
        let mut sum_p_tau2 = Matrix3::<f64>::zeros();
        let mut sum_target = Vector3::<f64>::zeros();
        let mut sum_target_tau = Vector3::<f64>::zeros();
        let mut terms = Vec::with_capacity(observations.len());
        for observation in observations {
            let direction = observation.direction.try_normalize(f64::EPSILON)?;
            let tau = seconds_between(epoch, observation.time);
            // 視線に垂直な成分だけを取り出す射影。視線と点の距離は、これを掛けたあとの長さになる。
            let projection = Matrix3::identity() - direction * direction.transpose();
            let target = observation.origin - gravity_displacement(tau);
            let projected_target = projection * target;

            sum_p += projection;
            sum_p_tau += projection * tau;
            sum_p_tau2 += projection * (tau * tau);
            sum_target += projected_target;
            sum_target_tau += projected_target * tau;
            terms.push((projection, target, tau));
        }
        let mut normal = Matrix6::<f64>::zeros();
        normal.fixed_view_mut::<3, 3>(0, 0).copy_from(&sum_p);
        normal.fixed_view_mut::<3, 3>(0, 3).copy_from(&sum_p_tau);
        normal.fixed_view_mut::<3, 3>(3, 0).copy_from(&sum_p_tau);
        normal.fixed_view_mut::<3, 3>(3, 3).copy_from(&sum_p_tau2);
        let mut rhs = Vector6::<f64>::zeros();
        rhs.fixed_rows_mut::<3>(0).copy_from(&sum_target);
        rhs.fixed_rows_mut::<3>(3).copy_from(&sum_target_tau);

        let singular_values = normal.singular_values();
        let largest = singular_values.max();
        if largest <= 0.0 || singular_values.min() / largest < MIN_RECIPROCAL_CONDITION {
            return None;
        }
        // 特異値は、解が定まっているかの判定にだけ使う。特異値分解から解を作ると、分解の誤差が小さい特異値の
        // 向きに拡大されて出ることがあった (視線 27 本の当てはめで、速度が 2 mm/s ずれた)。
        let solution = normal.cholesky()?.solve(&rhs);
        let position = solution.fixed_rows::<3>(0).into_owned();
        let velocity = solution.fixed_rows::<3>(3).into_owned();

        let squared_error: f64 = terms
            .iter()
            .map(|(projection, target, tau)| {
                (projection * (position + velocity * *tau - target)).norm_squared()
            })
            .sum();
        Some(Self {
            epoch,
            position,
            velocity,
            residual_rms: (squared_error / observations.len() as f64).sqrt(),
        })
    }

    pub fn position_at(&self, time: DateTime<Utc>) -> Vector3<f64> {
        let tau = seconds_between(self.epoch, time);
        self.position + self.velocity * tau + gravity_displacement(tau)
    }

    /// 的の面に向かう速さ (m/s)。遠ざかっているときは負になる。
    pub fn approach_speed(&self) -> f64 {
        -self.velocity.x
    }

    /// 的の面 (x = 0) を通る時刻と位置。的に向かっていないときは `None`。
    ///
    /// 重力は z 方向にしか働かないので、x は時刻の一次式になり、面を通る時刻は 1 つに決まる。
    pub fn plane_crossing(&self) -> Option<PlaneCrossing> {
        if self.velocity.x >= 0.0 {
            return None;
        }
        let tau = -self.position.x / self.velocity.x;
        let point = self.position + self.velocity * tau + gravity_displacement(tau);
        Some(PlaneCrossing {
            time: self.epoch + TimeDelta::nanoseconds((tau * 1e9).round() as i64),
            y: point.y,
            z: point.z,
        })
    }

    /// 当てはめに使った最後の観測の時刻での速度 (m/s)。重力で変わるのは z の成分だけ。
    pub fn velocity(&self) -> Vector3<f64> {
        self.velocity
    }

    /// 視線と、その視線の時刻の軌道上の点との離れぐあい。視線の向きが決まらないときは `None`。
    pub fn ray_offset(&self, observation: &RayObservation) -> Option<RayOffset> {
        let direction = observation.direction.try_normalize(f64::EPSILON)?;
        let to_ball = self.position_at(observation.time) - observation.origin;
        let depth = to_ball.dot(&direction);
        Some(RayOffset {
            distance: (to_ball - direction * depth).norm(),
            depth,
        })
    }

    /// 視線 1 本の誤差の見積もり σ (m)。`observations` には当てはめに使った視線を渡す。
    ///
    /// 視線と軌道の距離の二乗和を、式の余りの数で割る。未知数 6 個に対して視線 1 本が式 2 本ぶんなので、
    /// 余りは 2n − 6。視線の本数で割る `residual_rms` は、4 本や 5 本だと寄せ集めの視線でも小さく出る。
    /// 余りの数で割れば、本数によらず同じ基準で比べられる。
    ///
    /// 余りがない (3 本以下)、または向きの決まらない視線があるときは `None`。
    pub fn noise(&self, observations: &[RayObservation]) -> Option<f64> {
        let spare_equations = (2 * observations.len())
            .checked_sub(6)
            .filter(|spare| *spare > 0)?;
        let mut squared_error = 0.0;
        for observation in observations {
            squared_error += self.ray_offset(observation)?.distance.powi(2);
        }
        Some((squared_error / spare_equations as f64).sqrt())
    }

    /// 視線と軌道の距離のうち、最も大きいもの (m)。
    ///
    /// 視線がない、または向きの決まらない視線があるときは `None`。
    pub fn max_residual(&self, observations: &[RayObservation]) -> Option<f64> {
        let mut largest: Option<f64> = None;
        for observation in observations {
            let distance = self.ray_offset(observation)?.distance;
            largest = Some(largest.map_or(distance, |current| current.max(distance)));
        }
        largest
    }
}

fn seconds_between(from: DateTime<Utc>, to: DateTime<Utc>) -> f64 {
    (to - from).as_seconds_f64()
}

fn gravity_displacement(tau: f64) -> Vector3<f64> {
    Vector3::new(0.0, 0.0, -0.5 * GRAVITY_ACCELERATION * tau * tau)
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;

    use super::*;

    const CAMERA_A: Vector3<f64> = Vector3::new(1.0, -0.6, 1.5);
    const CAMERA_B: Vector3<f64> = Vector3::new(1.2, 0.7, 1.9);

    fn time(millis: i64) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(1_790_000_000_000 + millis).unwrap()
    }

    /// x = 2.5 m から的に向かって投げたボールの、`millis` ミリ秒後の位置。
    fn ball_at(millis: i64) -> Vector3<f64> {
        let t = millis as f64 / 1000.0;
        Vector3::new(2.5, 0.1, 1.4)
            + Vector3::new(-8.0, 0.5, 1.0) * t
            + Vector3::new(0.0, 0.0, -0.5 * GRAVITY_ACCELERATION * t * t)
    }

    fn observe(camera: Vector3<f64>, millis: i64) -> RayObservation {
        RayObservation {
            time: time(millis),
            origin: camera,
            direction: ball_at(millis) - camera,
        }
    }

    #[test]
    fn recovers_the_crossing_from_cameras_with_unsynchronized_shutters() {
        // 2 台とも約 29fps で、シャッターは 13ms ずれている。ボールは 130ms 以降は映っていない。
        let observations = [
            observe(CAMERA_A, 20),
            observe(CAMERA_B, 33),
            observe(CAMERA_A, 55),
            observe(CAMERA_B, 68),
            observe(CAMERA_A, 90),
            observe(CAMERA_B, 103),
        ];

        let trajectory = BallisticTrajectory::fit(&observations).unwrap();
        let crossing = trajectory.plane_crossing().unwrap();

        // x = 2.5 - 8t = 0 となるのは t = 312.5ms
        let expected = ball_at(0)
            + Vector3::new(-8.0, 0.5, 1.0) * 0.3125
            + Vector3::new(0.0, 0.0, -0.5 * GRAVITY_ACCELERATION * 0.3125 * 0.3125);
        assert_relative_eq!(trajectory.residual_rms, 0.0, epsilon = 1e-9);
        assert_relative_eq!(trajectory.approach_speed(), 8.0, epsilon = 1e-9);
        assert_relative_eq!(expected.x, 0.0, epsilon = 1e-12);
        assert_relative_eq!(crossing.y, expected.y, epsilon = 1e-9);
        assert_relative_eq!(crossing.z, expected.z, epsilon = 1e-9);
        assert!((crossing.time - time(312)).abs() <= TimeDelta::milliseconds(1));
    }

    #[test]
    fn reports_how_far_the_rays_are_from_the_trajectory() {
        let mut observations = vec![
            observe(CAMERA_A, 20),
            observe(CAMERA_B, 33),
            observe(CAMERA_A, 55),
            observe(CAMERA_B, 68),
            observe(CAMERA_A, 90),
            observe(CAMERA_B, 103),
        ];
        // 1 本だけ、ボールとは別のものを見ている。
        observations[2].direction = Vector3::new(0.0, 1.0, 0.0);

        let trajectory = BallisticTrajectory::fit(&observations).unwrap();

        assert!(trajectory.residual_rms > 0.05);
    }

    #[test]
    fn needs_rays_at_more_than_one_time() {
        // 位置は決まるが、速度が決まらない。
        let observations = [
            observe(CAMERA_A, 20),
            observe(CAMERA_B, 20),
            observe(CAMERA_A, 20),
            observe(CAMERA_B, 20),
        ];

        assert!(BallisticTrajectory::fit(&observations).is_none());
    }

    #[test]
    fn needs_enough_rays() {
        let observations = [
            observe(CAMERA_A, 20),
            observe(CAMERA_B, 33),
            observe(CAMERA_A, 55),
        ];

        assert!(BallisticTrajectory::fit(&observations).is_none());
    }

    #[test]
    fn does_not_cross_the_plane_when_moving_away() {
        // 的で跳ね返って戻ってくるボール。
        let bounce = |millis: i64| {
            let t = millis as f64 / 1000.0;
            Vector3::new(0.2, 0.0, 1.5) + Vector3::new(3.0, 0.0, 0.0) * t
                - Vector3::new(0.0, 0.0, 0.5 * GRAVITY_ACCELERATION * t * t)
        };
        let observations: Vec<_> = [
            (CAMERA_A, 0),
            (CAMERA_B, 13),
            (CAMERA_A, 35),
            (CAMERA_B, 48),
        ]
        .into_iter()
        .map(|(camera, millis)| RayObservation {
            time: time(millis),
            origin: camera,
            direction: bounce(millis) - camera,
        })
        .collect();

        let trajectory = BallisticTrajectory::fit(&observations).unwrap();

        assert!(trajectory.approach_speed() < 0.0);
        assert!(trajectory.plane_crossing().is_none());
    }
}
