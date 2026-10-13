#!/usr/bin/env python3
"""カメラの置き方ごとに、着弾位置の予測がどのくらい当たるかをシミュレーションで比べる。

    python simulate_layouts.py

投球をランダムに作り、各カメラの画角・フレームレート・露光中のぶれ・時計のずれ・取り付けの誤差を
入れて視線を作り、ball_tracker と同じ計算 (放物線を視線に当てはめて的の面まで延長する) で
着弾位置を予測する。誤差の大きさは 2026-10-07 の撮影で見えた値をもとにした仮定で、実測ではない。

座標は筐体データと同じ向きで、単位は m。x が奥行き (奥面が 0.0455)、y が左右、z が高さ。
必要なもの: Python 3、numpy。
"""
import numpy as np

GRAVITY = 9.80665
PLANE_X = 0.0455
# 筐体の内側。左右の壁、床、天井 (m)
Y_MIN, Y_MAX, Z_FLOOR, Z_CEILING = 0.04, 1.16, 1.10, 2.47
# 側面の壁が天井まであるのはここまで。これより手前はプレイヤー側で、背景が定まらない。
FRONT_X = 1.06

# 640 px 幅で焦点距離 482 px (KYV45)。長辺の画角は約 67 度
TAN_HALF_LONG = 320 / 482.2
TAN_HALF_SHORT = {"4:3": 240 / 482.2, "16:9": 180 / 482.2}

CLOCK_SIGMA = 0.005  # 2 台の時計のずれ (秒)
MOUNT_ANGLE_SIGMA = np.radians(2.0)  # 取り付けた向きの誤差
MOUNT_POSITION_SIGMA = 0.02  # 取り付けた位置の誤差 (m)
PIXEL_SIGMA = np.radians(0.3)  # ぶれがないときの、検出の中心の誤差
BLUR_FRACTION = 0.1  # ぶれた筋の長さに対する、中心の誤差の割合


def unit(v):
    v = np.asarray(v, float)
    return v / np.linalg.norm(v)


def camera(position, forward, long_axis):
    """位置、光軸の向き、画像の長辺を向ける方向からカメラを作る。"""
    forward = unit(forward)
    right = unit(np.asarray(long_axis, float) - np.dot(long_axis, forward) * forward)
    down = np.cross(forward, right)
    return {"position": np.asarray(position, float), "axes": np.array([right, down, forward])}


def corner_camera(x, y, z, towards_y, towards_z, turn_deg):
    """壁と床 (または天井) の角から、筐体の断面を斜めに見るカメラ。

    turn_deg は光軸を手前 (プレイヤー側) に振る角度。画角の半分だけ振ると、画像の奥側の端が
    奥面と平行になり、奥面 (プロジェクターの投影) をまったく映さなくなる。
    """
    across = unit([0.0, towards_y, towards_z])
    turn = np.radians(turn_deg)
    forward = np.cos(turn) * across + np.sin(turn) * np.array([1.0, 0.0, 0.0])
    return camera([x, y, z], forward, [0.0, -towards_z, towards_y])


def back_edge_camera(x, y, z, yaw_deg, pitch_deg):
    """奥面の端から、反対側の側面の壁を見るカメラ。スマホは縦持ち (画像の長辺が上下)。

    yaw_deg は真横からプレイヤー側に振った角度、pitch_deg は上向きを正とした角度。
    y が筐体の中央より大きければ左の壁を、小さければ右の壁を見る。
    """
    yaw, pitch = np.radians([yaw_deg, pitch_deg])
    side = -1.0 if y > (Y_MIN + Y_MAX) / 2 else 1.0
    forward = [np.sin(yaw) * np.cos(pitch), side * np.cos(yaw) * np.cos(pitch), np.sin(pitch)]
    return camera([x, y, z], forward, [0.0, 0.0, 1.0])


def posed_camera(x, y, z, yaw_deg, pitch_deg, roll_deg):
    """写真から求めた位置と向きのカメラ。スマホは縦持ち (画像の長辺がほぼ上下)。

    yaw_deg は真横 (反対側の壁の向き) からプレイヤー側に振った角度、pitch_deg は上向きを正とした
    角度、roll_deg は光軸まわりの傾き。y が筐体の中央より大きければ左の壁を、小さければ右の壁を見る。
    左右対称に置いたカメラは、y を中央で折り返し、roll の符号を反転して作る。
    """
    yaw, pitch, roll = np.radians([yaw_deg, pitch_deg, roll_deg])
    side = -1.0 if y > (Y_MIN + Y_MAX) / 2 else 1.0
    forward = np.array([np.sin(yaw) * np.cos(pitch), side * np.cos(yaw) * np.cos(pitch), np.sin(pitch)])
    level = unit(np.cross(forward, [0.0, 0.0, 1.0]))
    upward = np.cross(level, forward)
    short_axis = np.cos(roll) * level + np.sin(roll) * upward
    return camera([x, y, z], forward, np.cross(forward, short_axis))


def mirrored_pair(x, y, z, yaw_deg, pitch_deg, roll_deg):
    return [
        posed_camera(x, y, z, yaw_deg, pitch_deg, roll_deg),
        posed_camera(x, Y_MIN + Y_MAX - y, z, yaw_deg, pitch_deg, -roll_deg),
    ]


def background_is_stable(origin, direction):
    """その向きに見える背景が、壁・床・天井のどれかなら True。

    奥面にはプロジェクターの映像が映り、プレイヤー側は人が動くので、そこを背景にした
    ボールは背景との差分では拾えない。そういう画素はアプリ側で無視する前提で、観測から外す。
    """
    hits = []
    if direction[0] < 0:
        hits.append(((PLANE_X - origin[0]) / direction[0], False))
    if direction[0] > 0:
        hits.append(((FRONT_X - origin[0]) / direction[0], False))
    if direction[1] != 0:
        hits.append((((Y_MAX if direction[1] > 0 else Y_MIN) - origin[1]) / direction[1], True))
    if direction[2] != 0:
        hits.append((((Z_CEILING if direction[2] > 0 else Z_FLOOR) - origin[2]) / direction[2], True))
    return min(h for h in hits if h[0] > 0)[1]


def layouts(aspect):
    half_turn = np.degrees(np.arctan(TAN_HALF_SHORT[aspect]))
    u = unit([-100.0, 0.0, 154.0])  # いまスマホを立てかけている斜めの木の向き
    return {
        "いまの配置 (左右の壁で向かい合わせ)": [
            camera([0.564, Y_MIN, 1.795], [0, 1, 0], u),
            camera([0.564, Y_MAX, 1.795], [0, -1, 0], u),
        ],
        "左の壁 + 床 (真上を見る)": [
            camera([0.55, Y_MIN, 1.80], [0, 1, 0], [0, 0, 1]),
            camera([0.55, 0.60, Z_FLOOR], [0, 0, 1], [0, 1, 0]),
        ],
        "左の壁 + 天井 (真下を見る)": [
            camera([0.55, Y_MIN, 1.80], [0, 1, 0], [0, 0, 1]),
            camera([0.55, 0.60, Z_CEILING], [0, 0, -1], [0, 1, 0]),
        ],
        "床の左右の角から斜め上 (奥面を映さない向き)": [
            corner_camera(0.10, Y_MIN + 0.02, Z_FLOOR + 0.04, 1, 1, half_turn),
            corner_camera(0.10, Y_MAX - 0.02, Z_FLOOR + 0.04, -1, 1, half_turn),
        ],
        "床の左右の角から斜め上 (奥行きの中央、正面向き)": [
            corner_camera(0.55, Y_MIN + 0.02, Z_FLOOR + 0.04, 1, 1, 0.0),
            corner_camera(0.55, Y_MAX - 0.02, Z_FLOOR + 0.04, -1, 1, 0.0),
        ],
        # 2026-10-09 に仮置きしたカメラの写真から求めた位置と向きと、それを左右対称に置いたもの。
        "奥面の左右の端から反対側の壁 (10/9 の仮置き)": [
            back_edge_camera(0.105, 1.022, 1.695, 18.6, -6.9),
            back_edge_camera(0.105, Y_MIN + Y_MAX - 1.022, 1.695, 18.6, -6.9),
        ],
        # 床の角に置き直して撮った写真 (IMG_7069.jpg) から求めた位置と向き。
        "床の左右の角から斜め上 (10/9 の置き直し)": mirrored_pair(0.147, 1.065, 1.271, 25.3, 31.1, 11.4),
        # 向きを直して固定したあとの写真 (iPhone 13) から求めた位置と向き。レンズの高さは実測の
        # 床から 15cm に固定し、画角は写真から解いた。
        "床の左右の角から斜め上 (10/9 に固定)": mirrored_pair(0.065, 1.123, 1.265, 33.6, 43.7, 1.6),
        "天井の左右の角から斜め下 (奥行きの中央、正面向き)": [
            corner_camera(0.55, Y_MIN + 0.02, Z_CEILING - 0.03, 1, -1, 0.0),
            corner_camera(0.55, Y_MAX - 0.02, Z_CEILING - 0.03, -1, -1, 0.0),
        ],
    }


def fit_trajectory(observations):
    epoch = max(o[0] for o in observations)
    normal = np.zeros((6, 6))
    rhs = np.zeros(6)
    for time, origin, direction in observations:
        tau = time - epoch
        direction = direction / np.linalg.norm(direction)
        projection = np.eye(3) - np.outer(direction, direction)
        target = projection @ (origin - np.array([0.0, 0.0, -0.5 * GRAVITY * tau * tau]))
        normal[:3, :3] += projection
        normal[:3, 3:] += projection * tau
        normal[3:, :3] += projection * tau
        normal[3:, 3:] += projection * tau * tau
        rhs[:3] += target
        rhs[3:] += target * tau
    solution = np.linalg.solve(normal, rhs)
    return solution[:3], solution[3:], epoch


def small_rotation(rng, sigma):
    w = rng.normal(0, sigma, 3)
    k = np.array([[0, -w[2], w[1]], [w[2], 0, -w[0]], [-w[1], w[0], 0]])
    return np.eye(3) + k + 0.5 * k @ k


def simulate(cameras, fps, exposure, aspect, rng, trials=3000):
    half_short = TAN_HALF_SHORT[aspect]
    errors, frames, last_seen = [], [], []
    for _ in range(trials):
        speed = rng.uniform(4.0, 10.0)
        start = np.array([2.2, rng.uniform(0.3, 0.9), rng.uniform(1.5, 1.9)])
        hit = np.array([PLANE_X, rng.uniform(0.15, 1.05), rng.uniform(1.3, 2.3)])
        flight = (start[0] - PLANE_X) / speed
        velocity = (hit - start - np.array([0, 0, -0.5 * GRAVITY * flight ** 2])) / flight

        def ball(t):
            return start + velocity * t + np.array([0, 0, -0.5 * GRAVITY * t * t])

        observations, counts, latest = [], [], -1.0
        for cam in cameras:
            wrong_axes = cam["axes"] @ small_rotation(rng, MOUNT_ANGLE_SIGMA)
            wrong_position = cam["position"] + rng.normal(0, MOUNT_POSITION_SIGMA, 3)
            clock = rng.normal(0, CLOCK_SIGMA)
            count = 0
            for t in np.arange(rng.uniform(0, 1 / fps), flight, 1 / fps):
                seen = cam["axes"] @ (ball(t) - cam["position"])
                if seen[2] < 0.15:
                    continue
                image = seen[:2] / seen[2]
                if abs(image[0]) > 0.95 * TAN_HALF_LONG or abs(image[1]) > 0.95 * half_short:
                    continue
                if not background_is_stable(cam["position"], ball(t) - cam["position"]):
                    continue
                # 露光中に動いたぶんだけ筋になる。筋が長いほど、その中心は不確かになる。
                blur = np.linalg.norm(velocity + np.array([0, 0, -GRAVITY * t])) * exposure / np.linalg.norm(seen)
                sigma = np.hypot(PIXEL_SIGMA, BLUR_FRACTION * blur)
                noisy = np.array([image[0] + rng.normal(0, sigma), image[1] + rng.normal(0, sigma), 1.0])
                # 推定する側は、取り付けの誤差を知らないまま計算する。
                observations.append((t + clock, wrong_position, wrong_axes.T @ noisy))
                count += 1
                latest = max(latest, t)
            counts.append(count)
        frames.append(counts)
        if min(counts) < 2:
            continue
        try:
            position, vel, epoch = fit_trajectory(observations)
        except np.linalg.LinAlgError:
            continue
        if vel[0] >= 0:
            continue
        tau = (PLANE_X - position[0]) / vel[0]
        point = position + vel * tau + np.array([0, 0, -0.5 * GRAVITY * tau * tau])
        errors.append(point[1:] - hit[1:])
        last_seen.append(ball(latest)[0] - PLANE_X)
    errors = np.array(errors) * 100
    distance = np.linalg.norm(errors, axis=1)
    return {
        "predicted": len(errors) / trials,
        "frames": float(np.median(np.array(frames).min(axis=1))),
        "rms_y": float(np.sqrt((errors[:, 0] ** 2).mean())) if len(errors) else float("nan"),
        "rms_z": float(np.sqrt((errors[:, 1] ** 2).mean())) if len(errors) else float("nan"),
        "within_10": float((distance <= 10).mean() * len(errors) / trials) if len(errors) else 0.0,
        "last_seen": float(np.median(last_seen) * 100) if last_seen else float("nan"),
    }


def main():
    scenarios = [
        ("29fps・自動露出 (1/30 秒)", 29.0, 1 / 30, "4:3"),
        ("29fps・短い露光 (1/250 秒)", 29.0, 1 / 250, "4:3"),
        ("120fps (1/120 秒)", 120.0, 1 / 120, "16:9"),
    ]
    for title, fps, exposure, aspect in scenarios:
        print(f"\n== {title}、画像は {aspect}")
        print("配置                                  予測できた  少ない側のコマ数  誤差 y  誤差 z  10cm 以内  最後に見えた位置")
        for name, cameras in layouts(aspect).items():
            r = simulate(cameras, fps, exposure, aspect, np.random.default_rng(1))
            print(
                f"{name:<26s}  {r['predicted']:8.0%}  {r['frames']:10.0f}  "
                f"{r['rms_y']:8.1f}  {r['rms_z']:6.1f}  {r['within_10']:8.0%}  奥面の {r['last_seen']:4.0f} cm 手前"
            )


if __name__ == "__main__":
    main()
