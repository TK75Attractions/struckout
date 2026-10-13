#!/usr/bin/env python3
"""筐体の線が映った画像から、カメラの向き (診断用に位置と焦点距離も) を求める。静止点で確かめる。

    python calibrate_camera.py pose <画像> --position X Y Z --focal-ratio 0.753 --use right_wall,top_beam.right --rotation ROLL PITCH YAW
    python calibrate_camera.py check <設定.json>

pose は、筐体の基準 (booth.json) の線を画像の縁に合わせ、カメラアプリの設定画面 (Camera Pose) に
入れる値を出す。既定ではレンズの位置と焦点距離を固定し、向きだけを解く。
check は、位置を測った印に当てたボールを 2 台の画像から三角測量し、測った位置との差を出す。

必要なもの: Python 3、numpy、opencv-python。手順と設定ファイルの書式は README.md を参照。
"""
import argparse
import itertools
import json
import sys
from dataclasses import dataclass
from pathlib import Path

import cv2
import numpy as np

DEFAULT_BOOTH = Path(__file__).with_name("booth.json")
DEFAULT_BALL_RADIUS = 0.035
EDGE_SAMPLES = 50         # 線 1 本につき、縁を探す場所の数
MIN_EDGE_GRADIENT = 3.0   # 明るさの変化がこれより緩い場所は縁とみなさない (階調 / px)
MIN_EDGE_POINTS = 8       # 縁の点がこれより少ない線は使わない
EDGE_SIGMA_PX = 0.3       # 縁の読み取りの確からしさ (px)
ALIGN_CANDIDATES = 12     # 向きを粗く探すとき、細かい画像で比べ直すために残す候補の数
MIN_BALL_CONTRAST = 12.0  # ボールと周りの色みの差 (Lab の a, b) がこれより小さいと、色では見つけられない
# 解く量の並び: 位置 x, y, z (m)、向きの補正 (回転ベクトル、rad)、焦点距離 (px)、歪み 1 項
POSITION, ROTATION, FOCAL, RADIAL = [0, 1, 2], [3, 4, 5], [6], [7]
PARAMETER_STEPS = np.array([1e-6, 1e-6, 1e-6, 1e-6, 1e-6, 1e-6, 1e-3, 1e-6])
LINE_COLORS = [(0, 255, 0), (255, 0, 255), (0, 128, 255), (255, 255, 0), (0, 0, 255), (255, 128, 0), (0, 255, 255)]


@dataclass
class Camera:
    """ピンホールカメラ。座標は筐体データ (m)。画像はセンサーの向きのままで、左上が (0, 0)。"""
    position: np.ndarray
    rcw: np.ndarray       # 筐体データの座標からカメラ座標 (x: 画像の右、y: 画像の下、z: 光軸) への回転
    focal: float          # 焦点距離 (px)
    center: np.ndarray    # 光学中心 (px)
    k1: float = 0.0       # 歪み 1 項。OpenCV と同じ定義で、歪んだ位置 = 歪みのない位置 × (1 + k1 r^2)


# --- 向きの表し方の換算 ---

def device_to_camera(sensor_orientation):
    """端末センサー座標 (x: 右、y: 上、z: 画面の手前) からカメラ座標への回転。

    カメラアプリの defaultBackCameraRotation と同じ。LENS_POSE_ROTATION を返す端末では
    アプリがそちらを使うので、この行列とは少し違う (KYV45 は返さない)。
    """
    if sensor_orientation % 90 != 0:
        sys.exit(f"センサーの向きは 90 の倍数で指定してください: {sensor_orientation}")
    cos, sin = [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)][(sensor_orientation % 360) // 90]
    return np.array([[cos, -sin, 0.0], [-sin, -cos, 0.0], [0.0, 0.0, -1.0]])


def pose_rotation(roll, pitch, yaw):
    """ball_tracker と同じ Rz(yaw) * Ry(pitch) * Rx(roll)。角度は度。"""
    r, p, y = np.radians([roll, pitch, yaw])
    rx = np.array([[1, 0, 0], [0, np.cos(r), -np.sin(r)], [0, np.sin(r), np.cos(r)]])
    ry = np.array([[np.cos(p), 0, np.sin(p)], [0, 1, 0], [-np.sin(p), 0, np.cos(p)]])
    rz = np.array([[np.cos(y), -np.sin(y), 0], [np.sin(y), np.cos(y), 0], [0, 0, 1]])
    return rz @ ry @ rx


def pose_angles(rcw, device_rotation):
    """カメラの向きを、アプリの Camera Pose に入れる roll, pitch, yaw (度) に直す。

    アプリは端末センサー座標の視線を送り、ball_tracker が pose_rotation で世界座標に回す。
    その回転は R = rcw^T * D (D は device_to_camera) にあたる。
    """
    r = rcw.T @ device_rotation
    yaw = np.arctan2(r[1, 0], r[0, 0])
    pitch = -np.arcsin(np.clip(r[2, 0], -1.0, 1.0))
    roll = np.arctan2(r[2, 1], r[2, 2])
    return np.degrees([roll, pitch, yaw])


def rcw_from_pose_angles(roll, pitch, yaw, device_rotation):
    """pose_angles の逆。"""
    return (pose_rotation(roll, pitch, yaw) @ device_rotation.T).T


# --- カメラの計算 ---

def distort(normalized, k1):
    return normalized * (1.0 + k1 * (normalized ** 2).sum(axis=1, keepdims=True))


def undistort(normalized, k1):
    """distort の逆。式では解けないので、繰り返しで求める。"""
    if k1 == 0.0:
        return normalized
    out = normalized.copy()
    for _ in range(20):
        out = normalized / (1.0 + k1 * (out ** 2).sum(axis=1, keepdims=True))
    return out


def project(camera, points):
    """筐体データの座標の点を画像に映す。画像上の位置 (px) と、光軸に沿った距離 (m) を返す。"""
    in_camera = (camera.rcw @ (np.atleast_2d(points) - camera.position).T).T
    normalized = in_camera[:, :2] / in_camera[:, 2:3]
    return camera.center + camera.focal * distort(normalized, camera.k1), in_camera[:, 2]


def pixel_rays(camera, pixels):
    """画像上の点を通る視線の向き (筐体データの座標、長さ 1) を返す。"""
    normalized = undistort((np.atleast_2d(pixels) - camera.center) / camera.focal, camera.k1)
    rays = np.column_stack([normalized, np.ones(len(normalized))]) @ camera.rcw
    return rays / np.linalg.norm(rays, axis=1, keepdims=True)


def line_residuals(camera, line, pixels):
    """縁の点から、画像に映した線までの距離 (px、符号つき)。歪みを取り除いた画像の上で測る。"""
    start = camera.rcw @ (line["from"] - camera.position)
    end = camera.rcw @ (line["to"] - camera.position)
    # カメラと線を含む平面の法線。画像に映した線は、この平面と画像の面が交わる線になる。
    normal = np.cross(start, end)
    normalized = undistort((pixels - camera.center) / camera.focal, camera.k1)
    return camera.focal * (normalized @ normal[:2] + normal[2]) / np.hypot(normal[0], normal[1])


def ray_line_closest(camera, line, pixels):
    """縁の点ごとに、視線が線に最も近づく場所までの距離 (m) と、線に沿った位置 (m、from が 0) を返す。"""
    rays = pixel_rays(camera, pixels)
    along = line["to"] - line["from"]
    along = along / np.linalg.norm(along)
    offset = camera.position - line["from"]
    b, d, e = rays @ along, rays @ offset, along @ offset
    denominator = np.maximum(1.0 - b * b, 1e-12)
    return (b * e - d) / denominator, (e - b * d) / denominator


def visible_polyline(camera, start, end, size):
    """線分のうち、カメラの前にあって画像に入る部分を、画像上の点の列で返す。映らなければ None。"""
    points = start + np.outer(np.linspace(0.0, 1.0, 201), end - start)
    in_camera = (camera.rcw @ (points - camera.position).T).T
    front = in_camera[:, 2] > 0.05
    normalized = in_camera[front, :2] / in_camera[front, 2:3]
    # 歪みの式は、画像から大きく外れた場所では折り返してしまうので、そこは映らないものとする。
    usable = np.abs(camera.k1) * (normalized ** 2).sum(axis=1) < 0.5
    pixels = np.full((len(points), 2), -1.0)
    pixels[np.flatnonzero(front)[usable]] = camera.center + camera.focal * distort(normalized[usable], camera.k1)
    inside = (pixels[:, 0] >= 0) & (pixels[:, 0] <= size[0] - 1) & (pixels[:, 1] >= 0) & (pixels[:, 1] <= size[1] - 1)
    best, run_start = None, None
    for index, flag in enumerate(np.append(inside, False)):
        if flag and run_start is None:
            run_start = index
        elif not flag and run_start is not None:
            if best is None or index - run_start > best[1] - best[0]:
                best = (run_start, index)
            run_start = None
    if best is None or best[1] - best[0] < 2:
        return None
    return pixels[best[0]:best[1]]


# --- 筐体の基準と画像 ---

def load_booth(path):
    """booth.json を読み、「グループ.名前」をキーにした線と点を返す。"""
    data = json.loads(Path(path).read_text(encoding="utf-8"))
    lines, points = {}, {}
    for group, content in data["groups"].items():
        normal = content.get("surface_normal")
        for name, item in content.get("lines", {}).items():
            lines[f"{group}.{name}"] = {
                "from": np.array(item["from"], float), "to": np.array(item["to"], float),
                "sigma": float(item["sigma_cm"]) / 100.0, "description": item.get("description", ""),
            }
        for name, item in content.get("points", {}).items():
            points[f"{group}.{name}"] = {
                "position": np.array(item["position"], float), "sigma": float(item["sigma_cm"]) / 100.0,
                "normal": None if normal is None else np.array(normal, float),
            }
    return data, lines, points


def select_names(all_names, spec):
    """「グループ」か「グループ.名前」をカンマで並べた指定を、線の名前の一覧に直す。"""
    chosen = []
    for token in spec.split(","):
        matched = [name for name in all_names if name == token or name.startswith(token + ".")]
        if not matched:
            sys.exit(f"筐体の基準に {token} がありません。使える名前: {', '.join(all_names)}")
        chosen += [name for name in matched if name not in chosen]
    return chosen


def read_image(path):
    """画像を BGR で読む。cv2.imread は日本語の入ったパスを読めないことがあるので使わない。"""
    image = cv2.imdecode(np.fromfile(str(path), np.uint8), cv2.IMREAD_COLOR)
    if image is None:
        sys.exit(f"画像を読めません: {path}")
    return image


def write_image(path, image):
    ok, encoded = cv2.imencode(Path(path).suffix, image, [cv2.IMWRITE_JPEG_QUALITY, 90])
    if not ok:
        sys.exit(f"画像を書き出せません: {path}")
    encoded.tofile(str(path))


def read_frame_info(image_path):
    """アプリが画像と同じ名前で保存する JSON (画像の大きさ、内部パラメータなど) を読む。無ければ空。"""
    path = Path(image_path).with_suffix(".json")
    if not path.exists():
        return {}
    try:
        info = json.loads(path.read_text(encoding="utf-8"))
    except ValueError:
        return {}
    return info if isinstance(info, dict) else {}


def edge_tolerance(width):
    """1 本の直線に乗っているとみなす幅 (px)。1920 px 幅で 1.2 px。"""
    return 1.2 * max(width / 1920.0, 0.5)


def default_search(width):
    """線のおおよその位置の両側、縁を探す幅 (px)。1920 px 幅で 10 px、640 px 幅で 4 px。

    広げすぎると、隣の平行な縁 (木のもう片方の縁など) が混ざる。
    """
    return max(4, int(round(10 * width / 1920.0)))


def smoothed_gray(image):
    """縁を探すための、軽くぼかした白黒の画像。小さい画像では細い線がつぶれないよう、ぼかしを弱くする。"""
    kernel = 5 if image.shape[1] >= 1280 else 3
    return cv2.GaussianBlur(cv2.cvtColor(image, cv2.COLOR_BGR2GRAY).astype(np.float32), (kernel, kernel), 0)


def refine_edge(gray, p, q, half_width, extend, tolerance):
    """おおよその線 p-q の近くで縁の点を拾い、1 本の直線に乗らない点を捨てる。

    線に沿った EDGE_SAMPLES か所で、線と直角に明るさを読み、変化が最も急な位置を縁とする。
    縁の点の列と、それらを通る直線からのばらつき (px) を返す。
    """
    height, width = gray.shape
    p, q = np.array(p, float), np.array(q, float)
    direction = (q - p) / np.linalg.norm(q - p)
    normal = np.array([-direction[1], direction[0]])
    middle, length = (p + q) / 2, np.linalg.norm(q - p) * extend
    offsets = np.arange(-half_width, half_width + 1)
    points = []
    for t in np.linspace(-length / 2, length / 2, EDGE_SAMPLES):
        center = middle + t * direction
        xs, ys = center[0] + offsets * normal[0], center[1] + offsets * normal[1]
        if xs.min() < 2 or ys.min() < 2 or xs.max() > width - 3 or ys.max() > height - 3:
            continue
        profile = cv2.remap(gray, xs.astype(np.float32).reshape(1, -1), ys.astype(np.float32).reshape(1, -1),
                            cv2.INTER_LINEAR).ravel()
        gradient = np.abs(np.gradient(profile))
        k = int(np.argmax(gradient))
        if gradient[k] < MIN_EDGE_GRADIENT:
            continue
        if 0 < k < len(gradient) - 1:
            # 山の頂上を、両隣との放物線で 1 px より細かく求める
            a, b, c = gradient[k - 1], gradient[k], gradient[k + 1]
            k = k + 0.5 * (a - c) / (a - 2 * b + c + 1e-9)
        points.append(center + (k - half_width) * normal)
    points = np.array(points).reshape(-1, 2)
    scatter = float("nan")
    for step in range(5):
        if len(points) < 3:
            return np.empty((0, 2)), float("nan")
        mean = points.mean(axis=0)
        _, _, vt = np.linalg.svd(points - mean)
        offset = (points - mean) @ vt[1]
        scatter = float(offset.std())
        if step < 4:
            points = points[np.abs(offset) < max(tolerance, 2.0 * offset.std())]
    return points, scatter


def search_edges(gray, camera, lines, names, half_width, tolerance):
    """位置と向きの見込みから線を画像に映し、その近くの縁を拾う。"""
    size = (gray.shape[1], gray.shape[0])
    observations, scatters = {}, {}
    for name in names:
        polyline = visible_polyline(camera, lines[name]["from"], lines[name]["to"], size)
        if polyline is None or np.linalg.norm(polyline[-1] - polyline[0]) < 0.05 * size[0]:
            continue
        points, scatter = refine_edge(gray, polyline[0], polyline[-1], half_width, 1.0, tolerance)
        if len(points) >= MIN_EDGE_POINTS:
            observations[name], scatters[name] = points, scatter
    return observations, scatters


def align_roughly(gray, camera, lines, names, span_deg):
    """見込みの向きの周りで、映した線が画像の縁に最もよく重なる向きを探す。

    線を 1 本ずつ近くの縁に合わせると、隣の平行な縁 (木のもう片方の縁など) を拾いやすい。そこで先に
    すべての線をまとめて回し、線と直角な向きの明るさの変化が線の上に最も多く乗る向きを選ぶ。
    粗く縮めた画像で広く探し、細かい画像で狭く探し直す。
    """
    count = 40
    points = np.concatenate([lines[name]["from"] + np.outer(np.linspace(0.0, 1.0, count),
                                                            lines[name]["to"] - lines[name]["from"]) for name in names])
    # 線の向きを画像の上で知るために、線に沿って 1 cm 先の点も映す
    ahead = np.concatenate([np.tile(0.01 * (lines[name]["to"] - lines[name]["from"])
                                    / np.linalg.norm(lines[name]["to"] - lines[name]["from"]), (count, 1))
                            for name in names])
    candidates = [camera.rcw]
    shrink = max(1.0, gray.shape[1] / 240.0)
    span = float(span_deg)
    while True:
        level = cv2.resize(gray, None, fx=1.0 / shrink, fy=1.0 / shrink, interpolation=cv2.INTER_AREA)
        level = cv2.GaussianBlur(level, (0, 0), 1.0)
        gx = cv2.Sobel(level, cv2.CV_32F, 1, 0, ksize=3) / 8.0
        gy = cv2.Sobel(level, cv2.CV_32F, 0, 1, ksize=3) / 8.0
        # はっきりした縁もかすかな縁も同じように数えるため、変化の大きさは上限で頭打ちにする
        cap = max(float(np.percentile(np.hypot(gx, gy), 95)), 1.0)
        # 縮めた画像の 1.5 px ぶんずつ回す
        step = np.degrees(1.5 * shrink / camera.focal)
        offsets = np.radians(np.arange(-span, span + step / 2, step))
        scored = []
        for base in candidates:
            for rotation_vector in itertools.product(offsets, repeat=3):
                candidate = cv2.Rodrigues(np.array(rotation_vector))[0] @ base
                start = (points - camera.position) @ candidate.T
                end = start + ahead @ candidate.T
                front = (start[:, 2] > 0.05) & (end[:, 2] > 0.05)
                start, end = start[front], end[front]
                p = camera.center + camera.focal * start[:, :2] / start[:, 2:3]
                direction = camera.center + camera.focal * end[:, :2] / end[:, 2:3] - p
                direction /= np.maximum(np.linalg.norm(direction, axis=1, keepdims=True), 1e-9)
                column = np.round((p[:, 0] + 0.5) / shrink - 0.5).astype(int)
                row = np.round((p[:, 1] + 0.5) / shrink - 0.5).astype(int)
                inside = (column >= 0) & (column < level.shape[1]) & (row >= 0) & (row < level.shape[0])
                column, row, direction = column[inside], row[inside], direction[inside]
                across = np.abs(gx[row, column] * -direction[:, 1] + gy[row, column] * direction[:, 0])
                scored.append((float(np.minimum(across / cap, 1.0).sum()), candidate))
        # 粗い画像では正しい向きが 1 位になるとは限らないので、互いに離れた上位の候補を残して細かい画像で比べ直す
        scored.sort(key=lambda item: -item[0])
        kept = []
        for score, candidate in scored:
            if all(np.trace(candidate @ other.T) < 1.0 + 2.0 * np.cos(np.radians(2.0 * step)) for _, other in kept):
                kept.append((score, candidate))
                if len(kept) == ALIGN_CANDIDATES:
                    break
        if shrink <= 1.0 or shrink <= gray.shape[1] / 960.0:
            return kept[0][1], kept[0][0] / len(points)
        candidates = [candidate for _, candidate in kept]
        shrink, span = max(1.0, shrink / 2.0), 1.5 * step


# --- 当てはめ ---

def camera_from_state(state, base_rcw, center):
    rotation = cv2.Rodrigues(np.asarray(state[3:6], float).reshape(3, 1))[0]
    return Camera(np.array(state[:3], float), rotation @ base_rcw, float(state[6]), center, float(state[7]))


def line_weights(camera, lines, observations, weighting):
    """線ごとの重み。

    line: 線 1 本を 1 つの観測として扱う。点の数で割り、線の確からしさ (m) を画像上の大きさ (px) に
    直したもので割る。確からしさが同じなら、線の重みは距離だけで決まり、点の数は効かない。
    point: 縁の点 1 つずつを同じ重み (1 px) で扱う。10/10 の値の再現用。
    """
    if weighting == "point":
        return {name: 1.0 for name in observations}
    weights = {}
    for name, pixels in observations.items():
        distance, _ = ray_line_closest(camera, lines[name], pixels)
        sigma_px = np.hypot(camera.focal * lines[name]["sigma"] / np.mean(np.abs(distance)), EDGE_SIGMA_PX)
        weights[name] = 1.0 / (sigma_px * np.sqrt(len(pixels)))
    return weights


def residual_vector(state, base_rcw, center, lines, observations, weights, prior):
    camera = camera_from_state(state, base_rcw, center)
    parts = [line_residuals(camera, lines[name], pixels) * weights[name] for name, pixels in observations.items()]
    if prior is not None:
        parts.append((camera.position - prior["position"]) / prior["sigma"])
    return np.concatenate(parts)


def levenberg_marquardt(function, state, free):
    """function(state) の二乗和を、free に挙げた要素だけを動かして最小にする。"""
    state = np.array(state, float)
    residual = function(state)
    cost = residual @ residual
    damping = 1e-3
    for _ in range(200):
        jacobian = np.empty((len(residual), len(free)))
        for column, index in enumerate(free):
            delta = np.zeros(len(state))
            delta[index] = PARAMETER_STEPS[index]
            jacobian[:, column] = (function(state + delta) - function(state - delta)) / (2 * delta[index])
        normal = jacobian.T @ jacobian
        gradient = jacobian.T @ residual
        while True:
            candidate, candidate_cost = state.copy(), np.inf
            try:
                candidate[free] += np.linalg.solve(normal + damping * np.diag(np.diag(normal) + 1e-12), -gradient)
                candidate_residual = function(candidate)
                candidate_cost = candidate_residual @ candidate_residual
            except np.linalg.LinAlgError:
                pass
            if np.isfinite(candidate_cost) and candidate_cost < cost:
                break
            damping *= 4
            if damping > 1e12:
                return state, residual
        gain = cost - candidate_cost
        state, residual, cost, damping = candidate, candidate_residual, candidate_cost, max(damping / 4, 1e-9)
        if gain < 1e-14 * cost:
            break
    return state, residual


def fit_camera(start, lines, observations, free, weighting, prior):
    """start から始めて、free の自由度だけを動かして、映した線を縁の点に合わせる。"""
    state = np.concatenate([start.position, [0.0, 0.0, 0.0], [start.focal, start.k1]])
    # 線の重みは距離と焦点距離で決まるので、解くたびに見直す。point では変わらないので 1 回でよい。
    for _ in range(1 if weighting == "point" else 3):
        weights = line_weights(camera_from_state(state, start.rcw, start.center), lines, observations, weighting)
        state, _ = levenberg_marquardt(
            lambda s: residual_vector(s, start.rcw, start.center, lines, observations, weights, prior), state, free)
    return camera_from_state(state, start.rcw, start.center)


def misfit(camera, lines, observations, weighting):
    """当てはまりの悪さ (px^2)。出発点の違う解どうしを比べるのに使う。"""
    squares = [line_residuals(camera, lines[name], pixels) ** 2 for name, pixels in observations.items()]
    return float(sum(s.sum() if weighting == "point" else s.mean() for s in squares))


def in_front(camera, lines, observations):
    """縁の点の視線が、どの線にもカメラの前で届いているか。"""
    return all(np.median(ray_line_closest(camera, lines[name], pixels)[0]) > 0
               for name, pixels in observations.items())


def align_rotation(world, image):
    """world の向きを image の向きに重ねる回転 (Kabsch)。"""
    u, _, vt = np.linalg.svd(image.T @ world)
    return u @ np.diag([1.0, 1.0, np.linalg.det(u @ vt)]) @ vt


def starting_rotations(position, focal, center, lines, observations):
    """向きの出発点の候補を、縁の点だけから作る。

    線ごとに「カメラと線を含む平面」の法線が、筐体データの座標 (線とカメラの位置から) と
    カメラ座標 (縁の点から) の両方で決まる。この 2 組を重ねる回転が向きになる。
    法線の符号は決まらないので、組み合わせをすべて試して候補にする。
    """
    names = sorted(observations, key=lambda name: -len(observations[name]))
    world, image = [], []
    for name in names:
        normal = np.cross(lines[name]["from"] - position, lines[name]["to"] - position)
        world.append(normal / np.linalg.norm(normal))
        rays = np.column_stack([(observations[name] - center) / focal, np.ones(len(observations[name]))])
        _, _, vt = np.linalg.svd(rays / np.linalg.norm(rays, axis=1, keepdims=True))
        image.append(vt[2])
    world, image = np.array(world), np.array(image)
    tried = min(len(names), 5)
    candidates = []
    for signs in itertools.product((1.0, -1.0), repeat=tried):
        signs = np.array(signs)
        rotation = align_rotation(world[:tried], image[:tried] * signs[:, None])
        if len(names) > tried:
            rest = np.sign(np.einsum("ij,ij->i", image[tried:], world[tried:] @ rotation.T))
            rotation = align_rotation(world, image * np.concatenate([signs, rest])[:, None])
        candidates.append(rotation)
    return candidates


def solve_pose(position, focals, center, guess_rcw, lines, observations, free, weighting, prior):
    """出発点をいくつか試し、最もよく合った解を返す。"""
    best = None
    for focal in focals:
        rotations = starting_rotations(position, focal, center, lines, observations)
        if guess_rcw is not None:
            rotations.insert(0, guess_rcw)
        starts = [Camera(position, rotation, focal, center) for rotation in rotations]
        starts = [camera for camera in starts if in_front(camera, lines, observations)]
        starts.sort(key=lambda camera: misfit(camera, lines, observations, weighting))
        for start in starts[:3]:
            camera = fit_camera(start, lines, observations, free, weighting, prior)
            if camera.focal <= 0 or not in_front(camera, lines, observations):
                continue
            score = misfit(camera, lines, observations, weighting)
            if best is None or score < best[0]:
                best = (score, camera)
    if best is None:
        sys.exit("線に合う向きが見つかりませんでした。線の対応 (名前) と、位置・焦点距離の値を確かめてください")
    return best[1]


def describe_line(camera, line, pixels):
    """線 1 本の残差を px と cm でまとめる。cm は、線までの距離で px を長さに直した値。"""
    residual = line_residuals(camera, line, pixels)
    distance, along = ray_line_closest(camera, line, pixels)
    to_cm = 100.0 * float(np.mean(distance)) / camera.focal
    return {
        "points": int(len(pixels)),
        "mean_px": float(residual.mean()), "rms_px": float(np.sqrt((residual ** 2).mean())),
        "mean_cm": float(residual.mean()) * to_cm, "rms_cm": float(np.sqrt((residual ** 2).mean())) * to_cm,
        "distance_m": float(np.mean(distance)),
        "along_m": [float(along.min()), float(along.max())],
    }


def estimate_spread(camera, lines, observations, free, weighting, prior, position_sigma, device_rotation):
    """基準が確からしさの分だけ違っていたら結果がどれだけ動くかを、動かして解き直して調べる。

    戻り値は、原因ごとの [roll, pitch, yaw, 視線の向き (度), x, y, z (cm), 焦点距離 (px)] の動き。
    """
    angles = pose_angles(camera.rcw, device_rotation)

    def change(lines_now=lines, shift=np.zeros(3), focal_scale=1.0):
        start = Camera(camera.position + (0.0 if prior else 1.0) * shift, camera.rcw, camera.focal * focal_scale,
                       camera.center, camera.k1)
        prior_now = {"position": prior["position"] + shift, "sigma": prior["sigma"]} if prior else None
        other = fit_camera(start, lines_now, observations, free, weighting, prior_now)
        turn = (pose_angles(other.rcw, device_rotation) - angles + 180.0) % 360.0 - 180.0
        whole = np.degrees(np.linalg.norm(cv2.Rodrigues(other.rcw @ camera.rcw.T)[0]))
        return np.concatenate([turn, [whole], 100.0 * (other.position - camera.position), [other.focal - camera.focal]])

    from_lines = np.zeros(8)
    for name in observations:
        line = lines[name]
        along = (line["to"] - line["from"]) / np.linalg.norm(line["to"] - line["from"])
        side = np.cross(along, [1.0, 0.0, 0.0] if abs(along[0]) < 0.9 else [0.0, 1.0, 0.0])
        side /= np.linalg.norm(side)
        # 線の両端をそれぞれ、線と直角な 2 方向に確からしさの分だけ動かす
        for end in ("from", "to"):
            for direction in (side, np.cross(along, side)):
                moved = dict(lines)
                moved[name] = {**line, end: line[end] + line["sigma"] * direction}
                from_lines += change(lines_now=moved) ** 2
    from_position = np.zeros(8)
    for axis in range(3):
        from_position += change(shift=position_sigma * np.eye(3)[axis]) ** 2
    spread = {"lines": np.sqrt(from_lines), "position": np.sqrt(from_position)}
    spread["total"] = np.sqrt(from_lines + from_position)
    if FOCAL[0] not in free:
        spread["focal_1pct"] = np.abs(change(focal_scale=1.01))
    return spread


# --- 線を手で選ぶ (画面を使う。ここだけ OpenCV のウィンドウが要る) ---

def open_window(image):
    zoom = min(3.0, 1500.0 / image.shape[1], 850.0 / image.shape[0])
    shown = cv2.resize(image, None, fx=zoom, fy=zoom, interpolation=cv2.INTER_AREA if zoom < 1 else cv2.INTER_LINEAR)
    clicks = []

    def on_mouse(event, x, y, flags, parameter):
        if event == cv2.EVENT_LBUTTONDOWN:
            # 拡大して表示した画像の画素の中心を、元の画像の座標に戻す
            clicks.append([(x + 0.5) / zoom - 0.5, (y + 0.5) / zoom - 0.5])

    try:
        cv2.namedWindow("calibrate_camera")
        cv2.setMouseCallback("calibrate_camera", on_mouse)
    except cv2.error:
        sys.exit("この OpenCV は画面に表示できません (opencv-python-headless など)。--pick は使えません")
    return shown, zoom, clicks


def show_and_wait(shown, zoom, clicks, label, hint, needed):
    """クリックを集める。needed 点以上で Enter (または空白) を押すと確定。False は「この項目を飛ばす」。"""
    while True:
        canvas = shown.copy()
        if hint is not None:
            cv2.polylines(canvas, [np.round((hint + 0.5) * zoom - 0.5).astype(np.int32)], False, (0, 255, 255), 1,
                          cv2.LINE_AA)
        for x, y in clicks:
            cv2.circle(canvas, (int(round((x + 0.5) * zoom - 0.5)), int(round((y + 0.5) * zoom - 0.5))), 5,
                       (0, 0, 255), 1, cv2.LINE_AA)
        text = f"{label}: click, then Enter. u: undo, Enter with no click: skip, Esc: quit"
        cv2.putText(canvas, text, (10, 24), cv2.FONT_HERSHEY_SIMPLEX, 0.6, (0, 0, 0), 3, cv2.LINE_AA)
        cv2.putText(canvas, text, (10, 24), cv2.FONT_HERSHEY_SIMPLEX, 0.6, (255, 255, 255), 1, cv2.LINE_AA)
        cv2.imshow("calibrate_camera", canvas)
        key = cv2.waitKey(30) & 0xFF
        if key == 27:
            cv2.destroyAllWindows()
            sys.exit("中止しました")
        if key == ord("u") and clicks:
            clicks.pop()
        if key in (13, 32):
            if not clicks:
                return False
            if len(clicks) >= needed:
                return True


def pick_lines(image, names, lines, guess, previous):
    """線ごとに、画像に映った縁の上の 2 点をクリックしてもらう。"""
    shown, zoom, clicks = open_window(image)
    size = (image.shape[1], image.shape[0])
    print("線ごとに、縁の上の離れた 2 点をクリックして Enter を押してください。")
    print("u: 1 点取り消す、何もクリックせずに Enter: その線を飛ばす (前に選んだ位置があれば残す)、Esc: 中止")
    picks = dict(previous)
    for name in names:
        clicks.clear()
        hint = None if guess is None else visible_polyline(guess, lines[name]["from"], lines[name]["to"], size)
        print(f"  {name}: {lines[name]['description']}")
        if show_and_wait(shown, zoom, clicks, name, hint, 2):
            picks[name] = {"image_points": [[round(v, 1) for v in point] for point in clicks[-2:]]}
    cv2.destroyAllWindows()
    return picks


def pick_circle(image, label):
    """ボールの輪郭の上を 3 点以上クリックしてもらい、円を当てはめて中心と半径を返す。飛ばしたら None。"""
    shown, zoom, clicks = open_window(image)
    print(f"  {label}: ボールの輪郭の上を 3 点以上クリックして Enter を押してください")
    done = show_and_wait(shown, zoom, clicks, label, None, 3)
    cv2.destroyAllWindows()
    if not done:
        return None
    points = np.array(clicks)
    # x^2 + y^2 = 2 a x + 2 b y + c を最小二乗で解く
    solution, *_ = np.linalg.lstsq(np.column_stack([2 * points, np.ones(len(points))]), (points ** 2).sum(axis=1),
                                   rcond=None)
    return solution[0], solution[1], float(np.sqrt(solution[2] + solution[0] ** 2 + solution[1] ** 2))


# --- pose ---

def draw_overlay(image, camera, lines, observations):
    """較正に使った線を色つきで、使っていない線を白で描く。白い線は、較正とは独立した確認になる。"""
    canvas = image.copy()
    size = (image.shape[1], image.shape[0])
    thickness = max(1, int(round(size[0] / 960)))
    scale = max(0.4, size[0] / 2400)
    colors = {name: LINE_COLORS[index % len(LINE_COLORS)] for index, name in enumerate(observations)}
    for name, line in lines.items():
        polyline = visible_polyline(camera, line["from"], line["to"], size)
        if polyline is None:
            continue
        color = colors.get(name, (255, 255, 255))
        cv2.polylines(canvas, [np.round(polyline).astype(np.int32)], False, color, thickness, cv2.LINE_AA)
        for x, y in observations.get(name, []):
            cv2.circle(canvas, (int(round(x)), int(round(y))), 2 * thickness, color, -1, cv2.LINE_AA)
        x, y = polyline[len(polyline) // 2]
        (text_width, text_height), _ = cv2.getTextSize(name, cv2.FONT_HERSHEY_SIMPLEX, scale, thickness)
        anchor = (int(np.clip(x + 6, 0, max(0, size[0] - text_width - 2))),
                  int(np.clip(y - 6, text_height + 2, size[1] - 4)))
        cv2.putText(canvas, name, anchor, cv2.FONT_HERSHEY_SIMPLEX, scale, (0, 0, 0), 3 * thickness, cv2.LINE_AA)
        cv2.putText(canvas, name, anchor, cv2.FONT_HERSHEY_SIMPLEX, scale, color, thickness, cv2.LINE_AA)
    return canvas


def run_pose(args):
    image = read_image(args.image)
    height, width = image.shape[:2]
    info = read_frame_info(args.image)
    # アプリは fx, fy, cx, cy をほかの項目と並べて書く。
    intrinsics = info.get("intrinsics") if isinstance(info.get("intrinsics"), dict) else info
    booth, lines, _ = load_booth(args.booth)
    orientation = args.sensor_orientation if args.sensor_orientation is not None else info.get("sensor_orientation", 90)
    device_rotation = device_to_camera(int(orientation))
    # 光学中心は、アプリと同じく画像の中央とする (アプリが別の値を使っているときは、保存された JSON の値)。
    if args.principal_point:
        center = np.array(args.principal_point, float)
    elif "cx" in intrinsics and "cy" in intrinsics:
        center = np.array([intrinsics["cx"], intrinsics["cy"]], float)
    else:
        center = np.array([width / 2.0, height / 2.0])
    previous = json.loads(args.previous.read_text(encoding="utf-8")) if args.previous else None
    if args.focal_px:
        focal = args.focal_px
    elif args.focal_ratio:
        focal = args.focal_ratio * width
    elif "fx" in intrinsics:
        focal = float(intrinsics["fx"])
    else:
        focal = previous["focal_ratio"] * width if previous else None
    if focal is None and not args.free_focal:
        sys.exit("焦点距離が分かりません。--focal-px か --focal-ratio で指定してください (診断で求めるなら --free-focal)")
    position = np.array(args.position, float)
    position_sigma = args.position_tolerance_cm / 100.0
    names = select_names(list(lines), args.use) if args.use else list(lines)
    gray = smoothed_gray(image)
    tolerance = edge_tolerance(width)
    guess, search_lines = None, lines
    if previous:
        # 前の結果から、おおよその向きと、使った線、線ごとに縁が見えていた範囲を引き継ぐ。
        # 線は一部しか縁として見えないことが多く、見えていた範囲だけを探すほうが取り違えにくい。
        seen = {name: stats["along_m"] for name, stats in previous["lines"].items()
                if stats.get("used") and name in lines}
        if not args.use:
            names = list(seen)
        search_lines = dict(lines)
        for name, (start, end) in seen.items():
            along = (lines[name]["to"] - lines[name]["from"]) / np.linalg.norm(lines[name]["to"] - lines[name]["from"])
            search_lines[name] = {**lines[name], "from": lines[name]["from"] + start * along,
                                  "to": lines[name]["from"] + end * along}
        guess = Camera(position, np.array(previous["rcw"], float), focal or 0.8 * width, center)
    if args.rotation:
        guess = Camera(position, rcw_from_pose_angles(*args.rotation, device_rotation), focal or 0.8 * width, center)

    # 線のおおよその位置は、JSON (--lines) か、手で選ぶ (--pick) か、位置と向きの見込みから映すかで決める。
    lines_path = args.lines or args.image.with_name(args.image.stem + "_lines.json")
    picks = None
    if lines_path.exists() and (args.lines or args.pick):
        picks = json.loads(lines_path.read_text(encoding="utf-8"))["lines"]
        unknown = [name for name in picks if name not in lines]
        if unknown:
            sys.exit(f"{lines_path} に、筐体の基準に無い線があります: {', '.join(unknown)}")
    elif args.lines and not args.pick:
        sys.exit(f"{lines_path} がありません")
    if args.pick:
        picks = pick_lines(image, names, lines, guess, picks or {})
        lines_path.write_text(json.dumps({"image_size": [width, height], "lines": picks}, ensure_ascii=False, indent=2),
                              encoding="utf-8")
        print(f"線のおおよその位置を {lines_path} に書き出しました。次からは --lines {lines_path} で同じ位置を使えます")
    observations, scatters = {}, {}
    if picks is not None:
        print(f"線のおおよその位置: {lines_path}")
        for name in names:
            if name not in picks:
                continue
            p, q = picks[name]["image_points"]
            half_width = int(picks[name].get("search_px", args.search_px or default_search(width)))
            points, scatter = refine_edge(gray, p, q, half_width, float(picks[name].get("extend", 1.0)), tolerance)
            if len(points) >= MIN_EDGE_POINTS:
                observations[name], scatters[name] = points, scatter
            else:
                print(f"  {name}: 縁が見つかりませんでした (点 {len(points)} 個)。この線は使いません")
    elif guess is None or focal is None:
        sys.exit("線のおおよその位置が分かりません。--previous で前の結果を渡すか、--rotation でおおよその向きと"
                 "焦点距離を指定するか、--pick で線をクリックしてください")
    else:
        # 見込みの向きの周りで線が縁に重なる向きを粗く探してから、線ごとに縁を拾って向きを解く。
        aligned, support = align_roughly(gray, guess, search_lines, names, args.search_deg)
        moved = np.degrees(np.linalg.norm(cv2.Rodrigues(aligned @ guess.rcw.T)[0]))
        print(f"線のおおよその位置: 位置と向きの見込みから映しました (見込みの向きから {moved:.1f} 度回したところで、"
              f"線の {100 * support:.0f}% が縁に重なりました)")
        guess = Camera(position, aligned, guess.focal, center)
        narrow = args.search_px or default_search(width)
        for half_width in (2 * narrow, narrow):
            observations, scatters = search_edges(gray, guess, search_lines, names, half_width, tolerance)
            if len(observations) < 2:
                sys.exit("見込みの向きの近くに、線の縁が 2 本以上見つかりませんでした。--rotation と --use を見直すか、"
                         "--pick で線をクリックしてください")
            guess = fit_camera(guess, lines, observations, ROTATION, "line", None)
        missing = [name for name in names if name not in observations]
        if missing:
            print(f"  縁が見つからず、使わなかった線: {', '.join(missing)}")

    free = ROTATION + (POSITION if args.free_position else []) + (FOCAL if args.free_focal else []) \
        + (RADIAL if args.radial else [])
    if 2 * len(observations) < len(free):
        sys.exit(f"線が足りません。解く量 {len(free)} 個に対して、線 {len(observations)} 本 (条件は線 1 本につき 2 つ)")
    prior = {"position": position, "sigma": position_sigma} if args.free_position else None
    focals = [] if focal is None else [focal]
    if args.free_focal:
        focals += [ratio * width for ratio in (0.5, 0.75, 1.0, 1.5)]
    camera = solve_pose(position, focals, center, None if guess is None else guess.rcw, lines, observations, free,
                        args.weights, prior)

    # --- 結果 ---
    fixed = [label for label, flag in (("位置", not args.free_position), ("焦点距離", not args.free_focal)) if flag]
    solved = ["向き"] + [label for label, flag in (("位置", args.free_position), ("焦点距離", args.free_focal),
                                                  ("歪み 1 項", args.radial)) if flag]
    print(f"\n画像: {args.image.name} ({width}x{height})")
    print(f"解いた量: {'、'.join(solved)}" + (f" ({'、'.join(fixed)}は固定)" if fixed else "")
          + ("。線ごとに同じ重み" if args.weights == "line" else "。縁の点ごとに同じ重み"))
    if 2 * len(observations) == len(free):
        print("注意: 条件の数が解く量と同じなので、残差は当てはまりの確認になりません")
    print("\n較正に使った線   点の数  縁のばらつき(px)  残差 平均, 二乗平均 (px)  同 (cm)  カメラからの距離(m)")
    line_stats = {}
    for name, pixels in observations.items():
        stats = describe_line(camera, lines[name], pixels)
        stats["used"], stats["edge_scatter_px"] = True, scatters[name]
        line_stats[name] = stats
        print(f"  {name:22s} {stats['points']:3d}  {scatters[name]:6.2f}   {stats['mean_px']:+7.2f} {stats['rms_px']:6.2f}"
              f"   {stats['mean_cm']:+6.2f} {stats['rms_cm']:5.2f}   {stats['distance_m']:5.2f}"
              + ("   縁がばらついている (別の縁が混ざっていないか、重ね描きで確かめること)" if scatters[name] > tolerance else ""))
    everything = np.concatenate([line_residuals(camera, lines[name], pixels) for name, pixels in observations.items()])
    rms_points = float(np.sqrt((everything ** 2).mean()))
    rms_lines = float(np.sqrt(np.mean([stats["rms_px"] ** 2 for stats in line_stats.values()])))
    print(f"残差の二乗平均: 点ごと {rms_points:.2f} px、線ごと {rms_lines:.2f} px")

    # 較正に使っていない線も、映した位置の近くに縁があれば、ずれを出す。
    checked = {}
    others, other_scatters = search_edges(gray, camera, lines, [n for n in lines if n not in observations],
                                          default_search(width), tolerance)
    if others:
        print("\n較正に使っていない線 (確認用。映した位置の近くの縁を機械的に拾った値なので、重ね描きで確かめること)")
        for name, pixels in others.items():
            stats = describe_line(camera, lines[name], pixels)
            stats["used"], stats["edge_scatter_px"] = False, other_scatters[name]
            checked[name] = stats
            print(f"  {name:22s} {stats['points']:3d}  {other_scatters[name]:6.2f}   {stats['mean_px']:+7.2f} "
                  f"{stats['rms_px']:6.2f}   {stats['mean_cm']:+6.2f} {stats['rms_cm']:5.2f}   {stats['distance_m']:5.2f}")

    roll, pitch, yaw = pose_angles(camera.rcw, device_rotation)
    back_face_x = float(booth["back_face_x_m"])
    tracker = camera.position - [back_face_x + args.ball_radius, 0.0, 0.0]
    ratio = camera.focal / width
    print("\n設定画面 (Camera Pose) に入れる値")
    print(f"  x      {tracker[0]:.4f}   (筐体データの x {camera.position[0]:.4f} から、奥面 {back_face_x:.4f} と"
          f"ボールの半径 {args.ball_radius:.4f} を引いた値)")
    print(f"  y      {tracker[1]:.4f}")
    print(f"  z      {tracker[2]:.4f}")
    print(f"  roll   {roll:.3f}")
    print(f"  pitch  {pitch:.3f}")
    print(f"  yaw    {yaw:.3f}")
    print(f"  Focal length  {ratio:.5f}   (画像の幅に対する比。この画像で {camera.focal:.2f} px、"
          f"{args.analysis_width} px 幅の解析画像で {ratio * args.analysis_width:.2f} px)")
    if args.radial:
        corner = np.hypot(width, height) / 2 / camera.focal
        print(f"  歪み 1 項 k1 = {camera.k1:+.4f} (画像の隅で {camera.focal * camera.k1 * corner ** 3:+.1f} px)。"
              "線だけからは安定して決まらないので目安")

    spread = estimate_spread(camera, lines, observations, free, args.weights, prior, position_sigma, device_rotation)
    print("\n推定の幅 (基準がその分だけ違っていたら、結果がどれだけ動くか)")
    print("  原因                                roll    pitch   yaw    視線の向き (度)"
          + ("   x      y      z (cm)" if args.free_position else "") + ("   焦点距離 (px)" if args.free_focal else ""))
    labels = [("lines", "基準の線の確からしさ (booth.json)"),
              ("position", f"レンズの位置 {args.position_tolerance_cm:g} cm"),
              ("total", "上の 2 つの合計"), ("focal_1pct", "(参考) 焦点距離が 1 % 違う")]
    for key, label in labels:
        if key not in spread:
            continue
        row = spread[key]
        print(f"  {label:32s} {row[0]:6.2f} {row[1]:7.2f} {row[2]:6.2f} {row[3]:8.2f}"
              + (f"      {row[4]:6.2f} {row[5]:6.2f} {row[6]:6.2f}" if args.free_position else "")
              + (f"   {row[7]:8.1f}" if args.free_focal else ""))

    out = args.out or args.image.with_name(args.image.stem + "_pose.json")
    overlay_path = out.with_suffix(".jpg")
    write_image(overlay_path, draw_overlay(image, camera, lines, observations))
    result = {
        "image": args.image.name, "image_size": [width, height], "booth": str(args.booth),
        "solved": {"position": args.free_position, "focal": args.free_focal, "radial": args.radial,
                   "weights": args.weights},
        "config_screen": {
            "x": round(float(tracker[0]), 4), "y": round(float(tracker[1]), 4), "z": round(float(tracker[2]), 4),
            "roll": round(float(roll), 3), "pitch": round(float(pitch), 3), "yaw": round(float(yaw), 3),
            "focal_ratio": round(float(ratio), 5),
        },
        "position_booth_m": camera.position.tolist(), "position_tracker_m": tracker.tolist(),
        "rotation_deg": [float(roll), float(pitch), float(yaw)], "rcw": camera.rcw.tolist(),
        "focal_px": camera.focal, "focal_ratio": float(ratio), "principal_point_px": camera.center.tolist(),
        "radial_k1": camera.k1, "sensor_orientation": int(orientation),
        "ball_radius_m": args.ball_radius, "back_face_x_m": back_face_x,
        "residual_rms_px": {"points": rms_points, "lines": rms_lines},
        "lines": {**line_stats, **checked},
        "spread": {key: dict(zip(["roll_deg", "pitch_deg", "yaw_deg", "direction_deg", "x_cm", "y_cm", "z_cm",
                                  "focal_px"], value.tolist())) for key, value in spread.items()},
        "position_tolerance_cm": args.position_tolerance_cm,
    }
    out.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"\n重ね描きを {overlay_path} に、結果を {out} に書き出しました。重ね描きは必ず目で確かめること")


# --- check ---

def load_pose(path):
    """pose が書き出した結果の JSON から、カメラと画像の大きさを読む。"""
    data = json.loads(Path(path).read_text(encoding="utf-8"))
    camera = Camera(np.array(data["position_booth_m"], float), np.array(data["rcw"], float), float(data["focal_px"]),
                    np.array(data["principal_point_px"], float), float(data.get("radial_k1", 0.0)))
    return camera, data["image_size"]


def find_ball(image, predicted, radius):
    """予測した位置の近くで、周りと色みの違う丸いかたまりを探し、外接円の中心と半径 (px) を返す。

    ボールが映るはずの位置と大きさはカメラの位置と向きから分かるので、その周りだけを見る。
    見つからなければ None。
    """
    height, width = image.shape[:2]
    reach = int(round(max(4 * radius, 0.06 * width)))   # 予測からこの距離までを探す
    pad = int(np.ceil(1.6 * radius)) + 2
    x0, y0 = max(0, int(predicted[0]) - reach - pad), max(0, int(predicted[1]) - reach - pad)
    x1, y1 = min(width, int(predicted[0]) + reach + pad + 1), min(height, int(predicted[1]) + reach + pad + 1)
    if x1 - x0 < 2 * pad or y1 - y0 < 2 * pad:
        return None
    # 明るさは影で変わるので、色み (Lab の a, b) だけを見る。
    chroma = cv2.cvtColor(image[y0:y1, x0:x1], cv2.COLOR_BGR2LAB)[:, :, 1:].astype(np.float32)
    # ボールの大きさの円の中と、それを囲む輪の色みの平均を、場所ごとに比べる。差が最も大きい場所がボール。
    ys, xs = np.mgrid[-pad:pad + 1, -pad:pad + 1]
    distance = np.hypot(xs, ys)
    disc = (distance <= 0.8 * radius).astype(np.float32)
    ring = ((distance >= 1.2 * radius) & (distance <= 1.6 * radius)).astype(np.float32)
    inside = cv2.filter2D(chroma, -1, disc / disc.sum(), borderType=cv2.BORDER_REFLECT)
    around = cv2.filter2D(chroma, -1, ring / ring.sum(), borderType=cv2.BORDER_REFLECT)
    contrast = np.linalg.norm(inside - around, axis=2)
    rows, columns = np.mgrid[y0:y1, x0:x1]
    contrast[np.hypot(columns - predicted[0], rows - predicted[1]) > reach] = 0.0
    row, column = np.unravel_index(int(np.argmax(contrast)), contrast.shape)
    if contrast[row, column] < MIN_BALL_CONTRAST:
        return None
    # 見つけた場所の周りで、輪の色みよりも円の中の色みに近い画素をボールとして、その外接円を取る。
    # 手で一部が隠れても外接円はあまり動かないが、半分も残っていないものは使わない。
    ball = np.linalg.norm(chroma - inside[row, column], axis=2) < np.linalg.norm(chroma - around[row, column], axis=2)
    ball &= np.hypot(columns - (column + x0), rows - (row + y0)) <= 1.8 * radius
    mask = cv2.morphologyEx(ball.astype(np.uint8), cv2.MORPH_OPEN, cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (3, 3)))
    contours, _ = cv2.findContours(mask, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_NONE)
    if not contours:
        return None
    contour = max(contours, key=cv2.contourArea)
    (cx, cy), r = cv2.minEnclosingCircle(contour)
    if not (0.6 * radius <= r <= 1.6 * radius) or cv2.contourArea(contour) < 0.5 * np.pi * r * r:
        return None
    return cx + x0, cy + y0, r


def triangulate(first, second):
    """2 本の視線 (出発点, 向き) に最も近い点と、視線どうしの離れ (m) を返す。"""
    (c1, d1), (c2, d2) = first, second
    offset = c1 - c2
    b, d, e = d1 @ d2, d1 @ offset, d2 @ offset
    denominator = max(1.0 - b * b, 1e-12)
    p1 = c1 + (b * e - d) / denominator * d1
    p2 = c2 + (e - b * d) / denominator * d2
    return (p1 + p2) / 2, float(np.linalg.norm(p1 - p2))


def expected_center(point, booth_points, groups, radius):
    """印に当てたボールの中心の位置。印の位置から、面の法線の向きに半径ぶん離れたところ。"""
    if "ball_center_m" in point:
        return np.array(point["ball_center_m"], float)
    normal = None
    if "mark" in point:
        if point["mark"] not in booth_points:
            sys.exit(f"筐体の基準に点 {point['mark']} がありません")
        mark, normal = booth_points[point["mark"]]["position"], booth_points[point["mark"]]["normal"]
    else:
        mark = np.array(point["mark_m"], float)
    if "normal" in point:
        normal = np.array(point["normal"], float)
    elif "surface" in point:
        normal = groups.get(point["surface"], {}).get("surface_normal")
        normal = None if normal is None else np.array(normal, float)
    if normal is None:
        sys.exit(f"点 {point.get('name', '')} は、印のある面 (surface) か法線 (normal) の指定が要ります")
    return mark + radius * normal / np.linalg.norm(normal)


def run_check(args):
    config = json.loads(args.config.read_text(encoding="utf-8"))
    base = args.config.parent
    booth, _, booth_points = load_booth(base / config["booth"] if "booth" in config else args.booth)
    radius = float(config.get("ball_radius_m", DEFAULT_BALL_RADIUS))
    if len(config["cameras"]) != 2:
        sys.exit("cameras には 2 台を書いてください")
    cameras = {entry["name"]: load_pose(base / entry["pose"]) for entry in config["cameras"]}
    if args.debug_dir:
        args.debug_dir.mkdir(parents=True, exist_ok=True)

    rows = []
    print("点  ボールの中心 測った位置 x, y, z (cm)  三角測量 x, y, z (cm)  差 x, y, z (cm)  差の大きさ  視線の離れ (cm)  中心の求め方")
    for number, point in enumerate(config["points"], start=1):
        label = point.get("name", str(number))
        expected = expected_center(point, booth_points, booth["groups"], radius)
        rays, sources = [], []
        for name, (camera, pose_size) in cameras.items():
            pixel = point.get("pixels", {}).get(name)
            source = "指定"
            if pixel is None:
                image = read_image(base / point["images"][name])
                # 位置と向きを解いた画像と大きさが違うときは、その比で座標を合わせる
                zoom = image.shape[1] / pose_size[0]
                predicted, depth = project(camera, expected)
                predicted = (predicted[0] + 0.5) * zoom - 0.5
                found = find_ball(image, predicted, camera.focal * zoom * radius / depth[0]) if depth[0] > 0 else None
                source = "色"
                if found is None and args.pick:
                    found, source = pick_circle(image, f"{label} / {name}"), "クリック"
                if args.debug_dir:
                    # 十字は、測った位置から予測したボールの中心。円は、見つけたボール。
                    canvas = image.copy()
                    for color, thickness in (((0, 0, 0), 3), ((255, 255, 255), 1)):
                        cv2.drawMarker(canvas, (int(round(predicted[0])), int(round(predicted[1]))), color,
                                       cv2.MARKER_CROSS, 16, thickness)
                        if found is not None:
                            cv2.circle(canvas, (int(round(found[0])), int(round(found[1]))), int(round(found[2])),
                                       color, thickness, cv2.LINE_AA)
                    write_image(args.debug_dir / f"check_{number:02d}_{name}.jpg", canvas)
                if found is None:
                    break
                pixel = [(found[0] + 0.5) / zoom - 0.5, (found[1] + 0.5) / zoom - 0.5]
            rays.append((camera.position, pixel_rays(camera, np.array(pixel, float))[0]))
            sources.append(source)
        if len(rays) < 2:
            print(f"{label}: ボールの中心が見つかりませんでした。pixels で画像上の位置を書くか、--pick でクリックしてください")
            continue
        measured, gap = triangulate(*rays)
        difference = 100.0 * (measured - expected)
        rows.append({"name": label, "expected_m": expected.tolist(), "measured_m": measured.tolist(),
                     "difference_cm": difference.tolist(), "ray_gap_cm": 100.0 * gap})
        print(f"{label}  {100 * expected[0]:7.1f} {100 * expected[1]:6.1f} {100 * expected[2]:6.1f}   "
              f"{100 * measured[0]:7.1f} {100 * measured[1]:6.1f} {100 * measured[2]:6.1f}   "
              f"{difference[0]:+6.1f} {difference[1]:+6.1f} {difference[2]:+6.1f}   {np.linalg.norm(difference):5.1f}   "
              f"{100 * gap:5.1f}   {'+'.join(sources)}")
    if not rows:
        sys.exit("確かめられた点がありません")
    differences = np.array([row["difference_cm"] for row in rows])
    print(f"\n{len(rows)} 点の差の平均: x {differences[:, 0].mean():+.1f}、y {differences[:, 1].mean():+.1f}、"
          f"z {differences[:, 2].mean():+.1f} cm。差の大きさの最大 {np.linalg.norm(differences, axis=1).max():.1f} cm、"
          f"視線の離れの最大 {max(row['ray_gap_cm'] for row in rows):.1f} cm")
    if args.out:
        args.out.write_text(json.dumps({"points": rows}, ensure_ascii=False, indent=2), encoding="utf-8")
        print(f"結果を {args.out} に書き出しました")


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)

    pose = commands.add_parser("pose", help="画像 1 枚から向き (診断用に位置と焦点距離も) を解く")
    pose.add_argument("image", type=Path, help="解析画像 (センサーの向きのまま)")
    pose.add_argument("--position", type=float, nargs=3, required=True, metavar=("X", "Y", "Z"),
                      help="レンズの位置 (筐体データの座標、m)。--free-position のときは出発点")
    pose.add_argument("--position-tolerance-cm", type=float, default=1.0,
                      help="レンズの位置の確からしさ (cm)。--free-position のときは、位置をこの幅で測った値に引き寄せる")
    pose.add_argument("--focal-px", type=float, help="焦点距離 (この画像での px)")
    pose.add_argument("--focal-ratio", type=float, help="焦点距離 (画像の幅に対する比)")
    pose.add_argument("--principal-point", type=float, nargs=2, metavar=("CX", "CY"), help="光学中心 (px)。既定は画像の中央")
    pose.add_argument("--sensor-orientation", type=int, help="端末の SENSOR_ORIENTATION。既定は 90")
    pose.add_argument("--booth", type=Path, default=DEFAULT_BOOTH, help="筐体の基準")
    pose.add_argument("--use", help="較正に使う線。グループか「グループ.名前」をカンマで並べる。既定はすべて")
    pose.add_argument("--previous", type=Path,
                      help="前に解いた結果の JSON。おおよその向き、使う線、線ごとに縁が見えていた範囲を引き継ぐ")
    pose.add_argument("--rotation", type=float, nargs=3, metavar=("ROLL", "PITCH", "YAW"),
                      help="おおよその向き (アプリの Camera Pose と同じ角度、度)")
    pose.add_argument("--lines", type=Path, help="線のおおよその位置の JSON (--pick が書き出す。手で書いてもよい)")
    pose.add_argument("--pick", action="store_true",
                      help="線ごとに 2 点をクリックして選ぶ (画面が要る)。--lines か <画像の名前>_lines.json に書き出す")
    pose.add_argument("--search-px", type=int, help="線のおおよその位置の両側、縁を探す幅 (px)。既定は 1920 px 幅で 10")
    pose.add_argument("--search-deg", type=float, default=3.0,
                      help="--rotation の向きの周り、線が縁に重なる向きを探す幅 (度)。広げると遅くなり、取り違えも増える")
    pose.add_argument("--free-position", action="store_true", help="位置も解く (診断用)")
    pose.add_argument("--free-focal", action="store_true", help="焦点距離も解く (診断用)")
    pose.add_argument("--radial", action="store_true", help="歪み 1 項も解く (診断用。目安にとどめる)")
    pose.add_argument("--weights", choices=("line", "point"), default="line",
                      help="line: 線 1 本を 1 つの観測として扱う (既定)。point: 縁の点ごとに同じ重み (10/10 の再現用)")
    pose.add_argument("--ball-radius", type=float, default=DEFAULT_BALL_RADIUS, help="ボールの半径 (m)")
    pose.add_argument("--analysis-width", type=int, default=640, help="アプリの解析画像の幅 (px)。焦点距離の表示に使う")
    pose.add_argument("--out", type=Path, help="結果の JSON。既定は <画像の名前>_pose.json。重ね描きは同じ名前の .jpg")
    pose.set_defaults(run=run_pose)

    check = commands.add_parser("check", help="位置を測った印に当てたボールを 2 台で三角測量する")
    check.add_argument("config", type=Path)
    check.add_argument("--booth", type=Path, default=DEFAULT_BOOTH, help="筐体の基準")
    check.add_argument("--pick", action="store_true", help="色で見つからないとき、ボールの輪郭をクリックする (画面が要る)")
    check.add_argument("--debug-dir", type=Path, help="見つけたボールを描き込んだ画像の保存先")
    check.add_argument("--out", type=Path, help="結果の JSON")
    check.set_defaults(run=run_check)

    args = parser.parse_args()
    args.run(args)


if __name__ == "__main__":
    main()
