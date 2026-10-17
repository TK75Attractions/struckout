#!/usr/bin/env python3
"""別角度から撮った動画から、ボールが奥面に当たった位置 (正解データ) を求める。

ball_tracker の着弾座標と比べるための基準を作る。カメラ 1 台の映像でも、奥面の上にある
位置の分かった点 (四隅や継ぎ目) が 4 点以上映っていれば、画像の座標を奥面上の座標に直せる。

    python landing_truth.py <動画> <設定.json> --out impacts.csv --debug-dir debug/

必要なもの: Python 3、numpy、opencv-python、PATH 上の ffmpeg。
設定ファイルの書式は README.md を参照。
"""
import argparse
import csv
import json
import subprocess
import sys
from pathlib import Path

import cv2
import numpy as np


def read_frame_times(video):
    """各コマの表示時刻 (秒) を、表示順に並べて返す。

    スマホの動画はコマの間隔が一定とは限らず、ヘッダーの fps も目安でしかないので、
    コマ番号と時刻の対応は記録されている時刻から取る。
    """
    cmd = [
        "ffprobe", "-v", "error", "-select_streams", "v:0",
        "-show_entries", "packet=pts_time", "-of", "csv=p=0", str(video),
    ]
    out = subprocess.run(cmd, stdout=subprocess.PIPE, check=True, text=True).stdout
    return np.sort(np.array([float(line.strip(",")) for line in out.split() if line.strip(",")]))


def read_frames(video, times, first_frame, count, roi, scale):
    """first_frame から count コマを、roi で切り出し scale 倍に縮めた BGR 画像として読む。"""
    x, y, w, h = roi
    out_w, out_h = int(round(w * scale)), int(round(h * scale))
    # -ss を入力の前に置くと、ffmpeg は指定時刻以降の最初のコマから正確に出力する。
    # コマの時刻ちょうどを指定すると丸めで 1 コマずれうるので、半コマ手前を指定する。
    start = max(0.0, times[first_frame] - 0.5 * np.median(np.diff(times)))
    cmd = [
        "ffmpeg", "-v", "error", "-hwaccel", "auto", "-ss", f"{start:.6f}", "-i", str(video),
        "-frames:v", str(count), "-an",
        "-vf", f"crop={w}:{h}:{x}:{y},scale={out_w}:{out_h}:flags=area",
        "-f", "rawvideo", "-pix_fmt", "bgr24", "-",
    ]
    raw = subprocess.run(cmd, stdout=subprocess.PIPE, check=True).stdout
    frame_bytes = out_w * out_h * 3
    n = len(raw) // frame_bytes
    return np.frombuffer(raw[: n * frame_bytes], np.uint8).reshape(n, out_h, out_w, 3)


def find_ball_candidates(frames, background, min_diameter, max_diameter):
    """背景と色合いが違い、ボールらしい大きさ・形・色のかたまりをコマごとに返す。

    明るさの差ではなく色合い (Lab の a, b) の差で切り出す。奥面に近づいたボールは自分の影と
    くっついて見えるが、影は明るさしか変えないので、色合いで見ればボールだけが残る。
    """
    kernel = cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (5, 5))
    background_ab = cv2.cvtColor(background, cv2.COLOR_BGR2LAB)[:, :, 1:]
    per_frame = []
    for frame in frames:
        frame_ab = cv2.cvtColor(frame, cv2.COLOR_BGR2LAB)[:, :, 1:]
        diff = cv2.absdiff(frame_ab, background_ab).max(axis=2)
        _, mask = cv2.threshold(diff, 10, 255, cv2.THRESH_BINARY)
        mask = cv2.morphologyEx(mask, cv2.MORPH_OPEN, kernel)
        mask = cv2.morphologyEx(mask, cv2.MORPH_CLOSE, kernel)
        n, labels, stats, centroids = cv2.connectedComponentsWithStats(mask)
        found = []
        for k in range(1, n):
            x, y, w, h, area = stats[k]
            diameter = 2.0 * np.sqrt(area / np.pi)
            if not (min_diameter <= diameter <= max_diameter):
                continue
            # 手や体の一部は細長いか、外接矩形をほとんど埋めない。
            # 動きでぶれたボールは進行方向に伸びるので、縦横比は緩めに見る。
            if max(w, h) > 2.2 * min(w, h) or area < 0.55 * w * h:
                continue
            region = labels[y:y + h, x:x + w] == k
            b, g, r = cv2.mean(frame[y:y + h, x:x + w], region.astype(np.uint8))[:3]
            # 肌と合板はどちらも r > g > b で色の差が小さい。ボールはもっと鮮やか。
            # 影に入った暗いボールも残すため、鮮やかさは明るさに対する比 (彩度) で見る。
            saturation = (max(b, g, r) - min(b, g, r)) / max(b, g, r, 1.0)
            if saturation < 0.18 or (r >= g >= b and (r - b) < 0.5 * r):
                continue
            found.append({
                "x": float(centroids[k][0]), "y": float(centroids[k][1]),
                "diameter": float(diameter), "bgr": (b, g, r),
            })
        per_frame.append(found)
    return per_frame


def longest_track(per_frame, max_step):
    """コマをまたいで近いもの同士をつなぎ、最も長い軌跡を返す。"""
    finished, live = [], []
    for index, candidates in enumerate(per_frame):
        used, still_live = set(), []
        for track in live:
            last = track[-1]
            if len(track) >= 2 and track[-1]["i"] - track[-2]["i"] == 1:
                predicted = (2 * last["x"] - track[-2]["x"], 2 * last["y"] - track[-2]["y"])
            else:
                predicted = (last["x"], last["y"])
            best, best_distance = None, max_step
            for k, c in enumerate(candidates):
                if k in used:
                    continue
                distance = min(
                    np.hypot(c["x"] - predicted[0], c["y"] - predicted[1]),
                    np.hypot(c["x"] - last["x"], c["y"] - last["y"]),
                )
                if distance < best_distance:
                    best, best_distance = k, distance
            if best is not None:
                used.add(best)
                track.append({**candidates[best], "i": index})
                still_live.append(track)
            elif index - last["i"] <= 3:
                still_live.append(track)
            else:
                finished.append(track)
        still_live += [[{**c, "i": index}] for k, c in enumerate(candidates) if k not in used]
        live = still_live
    finished += live
    return max(finished, key=len) if finished else []


def fit_line(t, p):
    """p(t) = a + v t を最小二乗で当てはめ、(a, v, 残差の二乗和) を返す。"""
    design = np.stack([np.ones_like(t), t], axis=1)
    coef, *_ = np.linalg.lstsq(design, p, rcond=None)
    residual = p - design @ coef
    return coef[0], coef[1], float((residual ** 2).sum())


def find_impact(track, gravity, near_plane, span=4, min_turn=0.5):
    """軌跡が最初に折れ曲がる点を着弾とみなし、その時刻と位置を求める。

    重力による曲がりを取り除いたうえで、折れ点の前後 span コマずつに等速直線を当てはめ、
    前後の速度の食い違いが最初に山になる場所を探す。奥面で跳ね返ったあと床でも跳ねるので、
    最初の山だけを見る。着弾の瞬間はコマとコマの間にあるので、2 本の直線が最も近づく時刻をとる。

    @param gravity 重力加速度を画像上のベクトルに直したもの (px / コマ^2)
    @param near_plane 軌跡の点が奥面に届いていそうかを返す関数。投げる前の腕の振りや
    手を離れた瞬間も折れ点に見えるので、奥面の外や手前にある点は候補から外す
    """
    t = np.array([p["i"] for p in track], float)
    xy = np.array([[p["x"], p["y"]] for p in track], float)
    straight = xy - 0.5 * np.outer(t ** 2, gravity)
    fits = {}
    for k in range(2, len(track) - 2):
        if not near_plane(track[k]):
            continue
        before = slice(max(0, k - span + 1), k + 1)
        after = slice(k + 1, min(len(track), k + 1 + span))
        a0, v0, e0 = fit_line(t[before], straight[before])
        a1, v1, e1 = fit_line(t[after], straight[after])
        speed = np.linalg.norm(v0 + gravity * t[k])
        fits[k] = {"cost": e0 + e1, "turn": np.linalg.norm(v1 - v0) / max(speed, 1e-6),
                   "k": k, "a0": a0, "v0": v0, "a1": a1, "v1": v1}
    # 遠ざかるボールは遠近のせいで画像上では減速して見え、それだけで 0.2〜0.4 の食い違いが出る。
    first = next((k for k in sorted(fits) if fits[k]["turn"] >= min_turn), None)
    if first is None:
        return None
    run = []
    for k in sorted(fits):
        if k >= first:
            if fits[k]["turn"] < min_turn:
                break
            run.append(fits[k])
    # 折れ点の隣で区切っても食い違いは大きく出る。山の頂上付近から、当てはまりが最も良い区切りを選ぶ。
    peak = max(f["turn"] for f in run)
    best = min((f for f in run if f["turn"] >= 0.85 * peak), key=lambda f: f["cost"])
    dv = best["v0"] - best["v1"]
    da = best["a1"] - best["a0"]
    t_hit = float(dv @ da / (dv @ dv))
    t_hit = float(np.clip(t_hit, t[best["k"]], t[best["k"] + 1]))
    bend = 0.5 * gravity * t_hit ** 2
    incoming = best["a0"] + best["v0"] * t_hit + bend
    outgoing = best["a1"] + best["v1"] * t_hit + bend
    return {
        "index": t_hit,
        "xy": incoming,
        "mismatch": float(np.linalg.norm(incoming - outgoing)),
        "turn": float(best["turn"]),
    }


def estimate_camera(image_points, plane_points, image_size, focal):
    """奥面の基準点からカメラの位置 (奥面の座標系) を求める。視差の補正に使う。

    正面に近い角度で撮った平面からは焦点距離が決まらないので、焦点距離は設定で与える。
    """
    object_points = np.array([[0.0, y, z] for y, z in plane_points], np.float64)
    image_points = np.asarray(image_points, np.float64)
    K = np.array([[focal, 0, image_size[0] / 2.0], [0, focal, image_size[1] / 2.0], [0, 0, 1]])
    _, rvec, tvec = cv2.solvePnP(object_points, image_points, K, None, flags=cv2.SOLVEPNP_IPPE)
    projected, _ = cv2.projectPoints(object_points, rvec, tvec, K, None)
    error = float(np.sqrt(((projected.reshape(-1, 2) - image_points) ** 2).sum(axis=1).mean()))
    R, _ = cv2.Rodrigues(rvec)
    return {"error": error, "position": (-R.T @ tvec).ravel()}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("video", type=Path)
    parser.add_argument("config", type=Path)
    parser.add_argument("--out", type=Path, default=Path("impacts.csv"))
    parser.add_argument("--debug-dir", type=Path, help="検出結果を描き込んだ画像の保存先")
    args = parser.parse_args()

    config = json.loads(args.config.read_text(encoding="utf-8"))
    times = read_frame_times(args.video)
    fps = 1.0 / float(np.median(np.diff(times)))
    roi = config["roi"]
    scale = float(config.get("scale", 1.0))
    ball_diameter_cm = float(config["ball_diameter_cm"])
    image_points = np.array([p["image"] for p in config["reference_points"]], np.float64)
    plane_points = np.array([p["plane_cm"] for p in config["reference_points"]], np.float64)
    bounds = config["plane_bounds_cm"]

    homography, _ = cv2.findHomography(image_points, plane_points, 0)
    reprojected = cv2.perspectiveTransform(image_points.reshape(-1, 1, 2), homography).reshape(-1, 2)
    fit_error = np.abs(reprojected - plane_points).max()
    print(f"基準点の当てはめ誤差: 最大 {fit_error:.2f} cm")
    middle = image_points.mean(axis=0)
    step = cv2.perspectiveTransform(np.array([[middle], [middle + [1, 0]]]), homography).reshape(2, 2)
    cm_per_px = float(np.linalg.norm(step[1] - step[0]))
    camera = estimate_camera(image_points, plane_points, config["image_size"], float(config["focal_px"]))
    print(
        f"カメラ位置の推定: 奥面から {camera['position'][0]:.0f} cm, "
        f"y={camera['position'][1]:.0f} cm, z={camera['position'][2]:.0f} cm "
        f"(再投影誤差 {camera['error']:.1f} px)"
    )

    def to_full(xy):
        """切り出して縮めた画像の座標を、元の動画の座標に戻す。"""
        return np.asarray(xy, float) / scale + [roi[0], roi[1]]

    ball_px = ball_diameter_cm / cm_per_px * scale
    # 奥面上で真下 (z が減る向き) に 1 cm 動いたときの、縮めた画像上の動き。
    down = cv2.perspectiveTransform(
        np.array([[plane_points.mean(axis=0)], [plane_points.mean(axis=0) - [0, 1]]]), np.linalg.inv(homography)
    ).reshape(2, 2)
    gravity = (down[1] - down[0]) * scale * 980.0 / fps ** 2
    rows = []
    for number, throw in enumerate(config["throws"], start=1):
        first, last = throw["frames"]
        frames = read_frames(args.video, times, first, last - first + 1, roi, scale)
        background = np.median(frames[::3], axis=0).astype(np.uint8)
        per_frame = find_ball_candidates(frames, background, 0.6 * ball_px, 3.0 * ball_px)
        track = longest_track(per_frame, max_step=2.5 * ball_px)

        def near_plane(point):
            y, z = cv2.perspectiveTransform(to_full([point["x"], point["y"]]).reshape(1, 1, 2), homography).ravel()
            inside = bounds["y"][0] <= y <= bounds["y"][1] and bounds["z"][0] <= z <= bounds["z"][1]
            # 奥面より手前にあるボールは、奥面に着いたときより大きく写る。
            return inside and point["diameter"] <= 1.3 * ball_px

        impact = find_impact(track, gravity, near_plane) if len(track) >= 6 else None
        if impact is None:
            print(f"#{number} コマ {first}-{last}: 着弾を特定できませんでした (軌跡 {len(track)} 点)")
            continue

        full_xy = to_full(impact["xy"])
        on_plane = cv2.perspectiveTransform(full_xy.reshape(1, 1, 2), homography).ravel()
        # 画像に写るのはボールの中心で、着弾の瞬間は奥面より半径ぶん手前にある。
        # 画像の点をそのまま奥面に写すと、カメラから見て奥にずれるので引き戻す。
        radius = ball_diameter_cm / 2.0
        cam = camera["position"]
        center = on_plane + (cam[1:] - on_plane) * (radius / cam[0])
        frame_at_impact = first + impact["index"]
        time_at_impact = float(np.interp(frame_at_impact, np.arange(len(times)), times))
        nearest = min(track, key=lambda p: abs(p["i"] - impact["index"]))
        b, g, r = np.mean([p["bgr"] for p in track], axis=0)
        inside = bounds["y"][0] <= center[0] <= bounds["y"][1] and bounds["z"][0] <= center[1] <= bounds["z"][1]
        rows.append({
            "throw": number,
            "frame": round(frame_at_impact, 2),
            "time_s": round(time_at_impact, 4),
            "y_cm": round(float(center[0]), 1),
            "z_cm": round(float(center[1]), 1),
            "inside_plane": int(inside),
            "fit_mismatch_cm": round(impact["mismatch"] / scale * cm_per_px, 1),
            "turn": round(impact["turn"], 2),
            "parallax_cm": round(float(np.linalg.norm(center - on_plane)), 1),
            "seen_diameter_cm": round(nearest["diameter"] / scale * cm_per_px, 1),
            "ball_rgb": f"{int(r)}/{int(g)}/{int(b)}",
            "track_points": len(track),
        })
        print(
            f"#{number} コマ {frame_at_impact:.1f} ({time_at_impact:.3f} 秒): "
            f"y={center[0]:.1f} cm, z={center[1]:.1f} cm"
        )

        if args.debug_dir:
            args.debug_dir.mkdir(parents=True, exist_ok=True)
            deviation = np.abs(frames.astype(np.int16) - background).max(axis=3)
            picked = deviation.argmax(axis=0)
            canvas = np.take_along_axis(frames, picked[None, :, :, None], axis=0)[0].copy()
            for p in track:
                color = (0, 255, 0) if p["i"] <= impact["index"] else (255, 0, 255)
                cv2.circle(canvas, (int(p["x"]), int(p["y"])), 2, color, -1)
            hit = (int(round(impact["xy"][0])), int(round(impact["xy"][1])))
            cv2.drawMarker(canvas, hit, (0, 0, 255), cv2.MARKER_CROSS, 24, 2)
            corners = ((image_points - [roi[0], roi[1]]) * scale).astype(int)
            for cx, cy in corners:
                cv2.circle(canvas, (int(cx), int(cy)), 5, (255, 128, 0), 1)
            cv2.imwrite(str(args.debug_dir / f"throw_{number:02d}.jpg"), canvas)

    if not rows:
        sys.exit("着弾を 1 つも特定できませんでした")
    with args.out.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
    print(f"{len(rows)} 球を {args.out} に書き出しました")


if __name__ == "__main__":
    main()
