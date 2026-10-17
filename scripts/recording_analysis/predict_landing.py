#!/usr/bin/env python3
"""2 台のカメラの検出結果から着弾位置を予測し、正解と比べる。

    python predict_landing.py <設定.json>

detect_objects.py の検出結果を視線に直し、ball_tracker と同じ計算 (放物線を視線に当てはめて
的の面まで延長する) で着弾位置を出す。計算は ball_tracker/src/tracking/trajectory.rs と
同じなので、そちらを変えたらこちらも合わせること。

焦点距離 (focal_px)、時刻の補正 (time_correction_ms)、行ごとの露光の時刻差 (rolling_shutter_skew_ms) は
カメラごとに書ける。検出 1 件の時刻は、フレームの時刻に、時刻の補正と
「行ごとの時刻差 × 検出の中心の縦位置 (画像の上端が 0、下端が 1)」を足したものになる。

必要なもの: Python 3、numpy。設定ファイルの書式は README.md を参照。
"""
import csv
import json
import sys
from pathlib import Path

import numpy as np

GRAVITY = 9.80665
# 端末センサー座標 (x: 右、y: 上、z: 画面の手前) からカメラ座標 (x: 画像の右、y: 画像の下、
# z: 光軸) への回転。カメラアプリの defaultBackCameraRotation(90) と同じ。
DEVICE_TO_CAMERA = np.array([[0.0, -1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, -1.0]])


def pose_rotation(roll, pitch, yaw):
    """ball_tracker と同じ Rz(yaw) * Ry(pitch) * Rx(roll)。角度は度。"""
    r, p, y = np.radians([roll, pitch, yaw])
    rx = np.array([[1, 0, 0], [0, np.cos(r), -np.sin(r)], [0, np.sin(r), np.cos(r)]])
    ry = np.array([[np.cos(p), 0, np.sin(p)], [0, 1, 0], [-np.sin(p), 0, np.cos(p)]])
    rz = np.array([[np.cos(y), -np.sin(y), 0], [np.sin(y), np.cos(y), 0], [0, 0, 1]])
    return rz @ ry @ rx


def load_cameras(config, base):
    """設定の cameras を読み、カメラごとの位置・向き・焦点距離・時刻の補正と、検出の一覧を返す。

    検出の time はフレームの時刻 (全カメラ共通の時計での秒)。時刻の補正と行ごとの時刻差は
    まだ足していないので、検出 1 件の時刻は detection_time で求めること。
    estimate_time_offset.py もこの関数で読む。
    """
    cameras = []
    for camera in config["cameras"]:
        # 焦点距離はカメラごとに書ける。書いていなければ全体の値を使う。
        focal = camera.get("focal_px", config.get("focal_px"))
        if focal is None:
            sys.exit(f"{camera['name']}: focal_px がありません (カメラごとに書くか、全体に 1 つ書く)")
        rows = []
        with (base / camera["detections"]).open(encoding="utf-8") as handle:
            for row in csv.DictReader(handle):
                w, h = int(row["width"]), int(row["height"])
                rows.append({
                    # 全カメラ共通の時計 (このスクリプトでは、正解を作った動画の先頭からの秒) に直す
                    "time": float(row["time_s"]) - camera["time_offset_s"],
                    "u": int(row["x"]) + w / 2, "v": int(row["y"]) + h / 2, "size": min(w, h), "area": w * h,
                })
        cameras.append({
            "name": camera["name"], "rows": rows,
            "position": np.array(camera["position_m"], float),
            "rotation": pose_rotation(*camera["rotation_deg"]),
            "focal": float(focal),
            # どちらも秒に直して持つ
            "time_correction": camera.get("time_correction_ms", 0.0) / 1000.0,
            "row_skew": camera.get("rolling_shutter_skew_ms", 0.0) / 1000.0,
        })
    return cameras


def detection_time(camera, row, image_height):
    """検出 1 件の時刻 (秒)。フレームの時刻に、時刻の補正と行ごとの露光の時刻差を足す。

    行ごとの時刻差は画像の上端の行と下端の行の差で、検出の中心の縦位置 (上端 0、下端 1) を掛けて足す。
    docs/ball_detection_plan_3_time_sync_calibration.md の A.5 (ball_tracker での決め方) と同じ。
    """
    return row["time"] + camera["time_correction"] + camera["row_skew"] * row["v"] / image_height


def ray_direction(camera, row, image_size):
    """検出の中心を通る視線の向き (カメラの位置を書いたのと同じ座標系)。"""
    width, height = image_size
    focal = camera["focal"]
    in_camera = np.array([(row["u"] - width / 2) / focal, (row["v"] - height / 2) / focal, 1.0])
    return camera["rotation"] @ (DEVICE_TO_CAMERA.T @ in_camera)


def fit_trajectory(observations):
    """(時刻, カメラ位置, 視線の向き) の集まりに放物線を当てはめる。

    最後の観測の時刻での位置と速度、その時刻、視線と軌道の距離の二乗平均平方根を返す。
    """
    epoch = max(o[0] for o in observations)
    normal = np.zeros((6, 6))
    rhs = np.zeros(6)
    terms = []
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
        terms.append((projection, target, tau))
    solution = np.linalg.solve(normal, rhs)
    position, velocity = solution[:3], solution[3:]
    squared = [np.sum((p @ (position + velocity * tau) - target) ** 2) for p, target, tau in terms]
    return position, velocity, epoch, float(np.sqrt(np.mean(squared)))


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    config_path = Path(sys.argv[1])
    config = json.loads(config_path.read_text(encoding="utf-8"))
    base = config_path.parent
    image_size = config["image_size"]
    plane_x = float(config["plane_x_m"])
    earliest, latest = config["window_s"]
    min_size = config["min_bbox_px"]

    cameras = load_cameras(config, base)
    for camera in cameras:
        if camera["time_correction"] or camera["row_skew"]:
            print(
                f"{camera['name']}: 時刻の補正 {1000 * camera['time_correction']:+.1f} ms、"
                f"行ごとの時刻差 {1000 * camera['row_skew']:+.1f} ms"
            )

    with (base / config["truth"]).open(encoding="utf-8") as handle:
        impacts = list(csv.DictReader(handle))

    errors, residuals = [], []
    print("球  視線の数  速さ(m/s)  予測 y, z (cm)    正解 y, z (cm)    誤差 y, z (cm)   時刻の誤差(ms)  残差(cm)")
    for impact in impacts:
        hit_time = float(impact["time_s"])
        truth = np.array([float(impact["y_cm"]), float(impact["z_cm"])])
        observations, counts = [], []
        for camera in cameras:
            # 1 フレームに複数の検出があるときは、いちばん大きいものをボールとみなす。
            # 同じフレームの検出は、補正を足す前の時刻 (row["time"]) が同じになる。
            by_frame = {}
            for row in camera["rows"]:
                time = detection_time(camera, row, image_size[1])
                if not (earliest <= time - hit_time <= latest) or row["size"] < min_size:
                    continue
                if row["time"] not in by_frame or row["area"] > by_frame[row["time"]]["area"]:
                    by_frame[row["time"]] = row
            counts.append(len(by_frame))
            for row in by_frame.values():
                time = detection_time(camera, row, image_size[1])
                observations.append((time, camera["position"], ray_direction(camera, row, image_size)))
        label = "+".join(str(c) for c in counts)
        if len(observations) < 4 or min(counts) == 0:
            print(f"{impact['throw']:>2}  {label:>7}   視線が足りません")
            continue
        position, velocity, epoch, residual = fit_trajectory(observations)
        if velocity[0] >= 0:
            print(f"{impact['throw']:>2}  {label:>7}   的に向かっていません")
            continue
        tau = (plane_x - position[0]) / velocity[0]
        point = position + velocity * tau + np.array([0.0, 0.0, -0.5 * GRAVITY * tau * tau])
        predicted = point[1:] * 100
        error = predicted - truth
        errors.append(error)
        residuals.append(residual)
        print(
            f"{impact['throw']:>2}  {label:>7}  {np.linalg.norm(velocity):8.1f}  "
            f"{predicted[0]:7.1f} {predicted[1]:6.1f}   {truth[0]:7.1f} {truth[1]:6.1f}   "
            f"{error[0]:+7.1f} {error[1]:+6.1f}   {1000 * (epoch + tau - hit_time):+10.0f}  {100 * residual:8.1f}"
        )
    if errors:
        errors = np.array(errors)
        print(
            f"\n{len(errors)} 球の誤差: y 平均 {errors[:, 0].mean():+.1f} 標準偏差 {errors[:, 0].std():.1f} cm、"
            f"z 平均 {errors[:, 1].mean():+.1f} 標準偏差 {errors[:, 1].std():.1f} cm"
        )
        rms = np.sqrt((errors ** 2).mean(axis=0))
        worst = np.abs(errors).max(axis=0)
        print(
            f"二乗平均 y {rms[0]:.1f} z {rms[1]:.1f} cm、最大 y {worst[0]:.1f} z {worst[1]:.1f} cm、"
            f"視線と軌道の距離 (残差) の平均 {100 * np.mean(residuals):.2f} cm"
        )


if __name__ == "__main__":
    main()
