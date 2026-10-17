#!/usr/bin/env python3
"""飛んでいるボールの検出結果から、2 台のカメラの時刻のずれと、行ごとの露光の時刻差を見積もる。

    python estimate_time_offset.py <設定.json>
    python estimate_time_offset.py <設定.json> --event apex

2 台の時刻が合っていれば、2 台の視線は 1 本の放物線に乗る。右のカメラ (設定した位置の y が大きいほう) の
時刻を少しずつ動かして、視線と軌道の距離がいちばん小さくなるずれを 1 球ごとに探し、球ごとの値、平均、
中央値、標準誤差 (標準偏差 ÷ 球数の平方根)、全球をまとめたときの最小を表示する。同じやり方で、
行ごとの露光の時刻差 (rolling_shutter_skew_ms) も 1 台ずつ動かして調べる。正解の着弾位置は使わない。

表示する「ずれ」は、右のカメラの time_correction_ms に入れる値
(docs/ball_detection_plan_3_time_sync_calibration.md の --time-trim-ms にあたる)。
設定に書いてある右のカメラの time_correction_ms は「いまの設定」の距離を出すのにだけ使い、探す範囲の
基準にはしない。左のカメラの time_correction_ms と、2 台の rolling_shutter_skew_ms は設定の値のまま使う。

行ごとの時刻差を変えると、いちばん合う時刻のずれも一緒に動く (検出 1 件の時刻が「行ごとの時刻差 ×
画像の上端からの縦位置」だけ遅くなるため)。そこで行ごとの時刻差を調べるときは、値ごとに時刻のずれを
合わせ直す。行ごとの時刻差を設定に入れるときは、そのとき表示されたずれと組にして入れること。
行ごとの時刻差は 1 台ずつしか動かさない (もう 1 台は設定の値のまま)。2 台の画像をボールが逆向きに
横切る置き方では、どちらのカメラに付けても距離が縮むので、どちらのカメラの値かは決めにくい
(2026-10-10 の録画では、左に付けても右に付けても 1.35 cm が 1.2 cm 台になり、2 つの差は 0.05 cm しか無い)。

設定ファイルは predict_landing.py や replay_to_tracker.py と同じ書式。検出の CSV にはボールの検出だけを
入れておくこと (ボール以外が混ざると結果が狂う。1 フレームに複数あるときは、いちばん大きいものを使う)。
検出が --gap-s 秒より長く途切れたところで、別の球に分ける。

求めたずれには、カメラの位置と向きの誤差も混ざる。2026-10-10 の録画では、右のカメラの位置を奥行き (x) に
1 cm 動かすと (向きはそのまま)、ずれが約 2.5 ms 変わった。

--event apex は、真上に軽く投げ上げたボールの頂点の時刻を 2 台で比べる (トス試験)。カメラごとに、
画像上の軌跡へ時刻の 2 次式を当てはめて頂点 (画像上の速度が加速度と直角になる瞬間) の時刻を出す。
カメラの位置と向きは使わない。投げ上げの録画がまだ無いので、実際のデータでは確かめていない。
計算で作った検出 (2026-10-10 の配置、120 fps、中心の誤差 0.5 から 1 px) では次のようになった。
  - 真上に上がってそのまま落ちるボールなら、1 回ごとのばらつきは 1 ms ほどで、数回の平均は 1 ms 以内で合う。
  - ボールが左右 (2 台を結ぶ向き) に流れると、頂点の時刻が 2 台で逆向きにずれる。0.1 m/s の流れで、
    求めたずれが約 12 ms 変わった (2026-10-07 のように水平に向かい合わせた配置でも約 7 ms)。
    奥行きの向きに流れても、ほとんど変わらない。
  - 流れの向きと大きさが 1 回ごとにばらつくとき、5 回の平均の誤差は、ばらつきが 0.03 m/s (0.5 秒で
    1.5 cm) なら 2 ms ほど、0.1 m/s なら 5 ms ほどになった。
  - 表示する「横の流れ」は、そのカメラから見て横切る向きの流れだけを表す。2 台が正面から向かい合う
    置き方では、問題になる左右の流れはどちらのカメラにも奥行きの動きとしてしか映らず、この列に出ない。
このため、平均が 2 ms まで合うのは、ボールがほとんど流れていない投げ上げだけを集めたときに限る。

必要なもの: Python 3、numpy。設定ファイルの書式は README.md を参照。
"""
import argparse
import json
import sys
from pathlib import Path

import numpy as np

import predict_landing

LEFT, RIGHT = 0, 1


def load_throws(config, base, gap):
    """検出を読み、[左, 右] の順に並べたカメラと、球ごとにまとめた検出を返す。"""
    cameras = predict_landing.load_cameras(config, base)
    if len(cameras) != 2:
        sys.exit("カメラを 2 台書いた設定が必要です")
    # camera id は接続の順で変わるので、左右は位置で決める (y が大きいほうが右)。
    cameras.sort(key=lambda camera: camera["position"][1])
    start, end = config.get("time_range_s", [float("-inf"), float("inf")])
    min_size = config.get("min_bbox_px", 0)

    detections = []
    for index, camera in enumerate(cameras):
        # 1 フレームに複数の検出があるときは、いちばん大きいものをボールとみなす (predict_landing.py と同じ)。
        by_frame = {}
        for row in camera["rows"]:
            if not (start <= row["time"] <= end) or row["size"] < min_size:
                continue
            if row["time"] not in by_frame or row["area"] > by_frame[row["time"]]["area"]:
                by_frame[row["time"]] = row
        detections += [(row["time"], index, row) for row in by_frame.values()]
    detections.sort(key=lambda detection: detection[:2])

    groups = []
    for detection in detections:
        if not groups or detection[0] - groups[-1][-1][0] > gap:
            groups.append([])
        groups[-1].append(detection)
    return cameras, [pack_throw(cameras, group, config["image_size"]) for group in groups]


def pack_throw(cameras, detections, image_size):
    """1 球ぶんの検出を、当てはめに使う配列にまとめる。配列はどれも検出 1 件が 1 行。"""
    camera = np.array([index for _, index, _ in detections])
    rows = [row for _, _, row in detections]
    direction = np.array([predict_landing.ray_direction(cameras[index], row, image_size)
                          for index, row in zip(camera, rows)])
    direction /= np.linalg.norm(direction, axis=1, keepdims=True)
    return {
        "camera": camera,
        # フレームの時刻 (秒)。時刻の補正と行ごとの時刻差は足していない
        "time": np.array([row["time"] for row in rows]),
        # 検出の中心の縦位置 (画像の上端が 0、下端が 1)。行ごとの時刻差はこれに比例して足す
        "row": np.array([row["v"] / image_size[1] for row in rows]),
        "pixel": np.array([[row["u"], row["v"]] for row in rows]),
        "origin": np.array([cameras[index]["position"] for index in camera]),
        # 視線に垂直な成分だけを取り出す射影。視線と点の距離は、これを掛けたあとの長さになる
        "projection": np.eye(3) - direction[:, :, None] * direction[:, None, :],
        "counts": [int((camera == LEFT).sum()), int((camera == RIGHT).sum())],
    }


def distance_rms(throw, times):
    """時刻の付け方 (times の 1 行が 1 通り) ごとに放物線を当てはめ、視線と軌道の距離の二乗平均平方根を返す。

    計算は predict_landing.fit_trajectory と同じ。時刻をずらしながら何千通りも解くので、まとめて解く。
    """
    projection = throw["projection"]
    # 桁落ちを避けるため、時刻はこの球の中ほどからの秒数で扱う。
    tau = times - throw["time"].mean()
    # 視線ごとの目標の点。カメラの位置を、重力で落ちるぶんだけ上にずらして射影したもの。
    target = (np.einsum("nij,nj->ni", projection, throw["origin"])
              + (0.5 * predict_landing.GRAVITY * tau ** 2)[:, :, None] * projection[:, :, 2])
    normal = np.empty((len(tau), 6, 6))
    normal[:, :3, :3] = projection.sum(axis=0)
    normal[:, :3, 3:] = normal[:, 3:, :3] = np.einsum("bn,nij->bij", tau, projection)
    normal[:, 3:, 3:] = np.einsum("bn,nij->bij", tau ** 2, projection)
    rhs = np.concatenate([target.sum(axis=1), (target * tau[:, :, None]).sum(axis=1)], axis=1)
    solution = np.linalg.solve(normal, rhs[:, :, None])[:, :, 0]
    point = solution[:, None, :3] + solution[:, None, 3:] * tau[:, :, None]
    miss = np.einsum("nij,bnj->bni", projection, point) - target
    return np.sqrt((miss ** 2).sum(axis=(1, 2)) / tau.shape[1])


def scan(throw, cameras, shifts, skews):
    """右のカメラの時刻のずれ shifts と、行ごとの時刻差の組 skews (1 行が [左, 右]) の組み合わせごとに、
    視線と軌道の距離 (m) を返す。行が skews、列が shifts。単位はどれも秒。
    """
    is_right = throw["camera"] == RIGHT
    # 左の時刻の補正は設定のまま。右は shifts で置き換える。
    times = throw["time"] + np.where(is_right, 0.0, cameras[LEFT]["time_correction"])
    times = times + skews[:, throw["camera"]] * throw["row"]
    times = times[:, None, :] + shifts[None, :, None] * is_right
    return distance_rms(throw, times.reshape(-1, times.shape[-1])).reshape(len(skews), len(shifts))


def refine_minimum(xs, ys):
    """等間隔の xs での値 ys から、最小の位置と値を放物線で補間して返す。

    最小が端にあるときは補間できないので、端の値のまま返し、3 つ目に True を返す。
    """
    k = int(np.argmin(ys))
    if k == 0 or k == len(xs) - 1:
        return float(xs[k]), float(ys[k]), True
    before, here, after = ys[k - 1], ys[k], ys[k + 1]
    curvature = before - 2 * here + after
    if curvature <= 0:
        return float(xs[k]), float(here), False
    offset = 0.5 * (before - after) / curvature
    return float(xs[k] + offset * (xs[1] - xs[0])), float(here - 0.25 * (before - after) * offset), False


def describe(values, counter):
    """球ごとの値 (ms) の平均、中央値、標準偏差、標準誤差を 1 行にまとめる。"""
    values = np.array(values)
    text = f"{len(values)} {counter}: 平均 {values.mean():+.1f} ms、中央値 {np.median(values):+.1f} ms"
    if len(values) >= 2:
        deviation = values.std(ddof=1)
        text += f"、標準偏差 {deviation:.1f} ms、標準誤差 {deviation / np.sqrt(len(values)):.1f} ms"
    return text


def time_span(throw):
    return f"{throw['time'].min():.3f}-{throw['time'].max():.3f}".ljust(17)


def frame_counts(throw):
    return f"{throw['counts'][LEFT]}+{throw['counts'][RIGHT]}"


def step_range(low, high, step):
    return np.arange(low, high + step / 2, step)


def report_time_shift(cameras, used, args):
    """右のカメラの時刻のずれを動かして、球ごと・全球でいちばん合う値を表示する。"""
    shifts = step_range(*args.range_ms, args.step_ms)
    skews = np.array([[cameras[LEFT]["row_skew"], cameras[RIGHT]["row_skew"]]])
    current = 1000 * cameras[RIGHT]["time_correction"]
    curves = [100 * scan(throw, cameras, shifts / 1000, skews)[0] for _, throw in used]
    now = [100 * scan(throw, cameras, np.array([current / 1000]), skews)[0, 0] for _, throw in used]

    print("\n球  時刻(秒)          コマ数(左+右)  合うずれ(ms)  そのときの距離(cm)  いまの設定での距離(cm)")
    best = []
    for (number, throw), curve, distance_now in zip(used, curves, now):
        shift, distance, at_edge = refine_minimum(shifts, curve)
        if not at_edge:
            best.append(shift)
        print(
            f"{number:>2}  {time_span(throw)}  {frame_counts(throw):>9}      {shift:+8.1f}  {distance:14.2f}  "
            f"{distance_now:16.2f}{'   範囲の端 (下の集計から除く)' if at_edge else ''}"
        )
    if best:
        print("\n" + describe(best, "球"))
    else:
        print("\nどの球も範囲の端で最小になりました。--range-ms を広げてください")
    shift, distance, at_edge = refine_minimum(shifts, np.mean(curves, axis=0))
    print(
        f"全球をまとめた最小 (球ごとの距離の平均が最小): {shift:+.1f} ms で {distance:.2f} cm"
        f"{' (範囲の端。--range-ms を広げてください)' if at_edge else ''}。"
        f"いまの設定 ({current:+.1f} ms) では {np.mean(now):.2f} cm"
    )


def best_shift_per_skew(table, shifts):
    """行ごとの時刻差の値ごとに、時刻のずれを合わせ直したときのずれと距離を返す。"""
    refined = [refine_minimum(shifts, curve) for curve in table]
    return np.array([r[0] for r in refined]), np.array([r[1] for r in refined])


def report_row_skew(cameras, used, args):
    """行ごとの露光の時刻差を 1 台ずつ動かして、球ごと・全球でいちばん合う値を表示する。"""
    values = step_range(*args.skew_range_ms, args.step_ms)
    # 行ごとの時刻差を変えると、いちばん合う時刻のずれも動く。ずれを探す範囲をそのぶん広げておく。
    reach = np.abs(args.skew_range_ms).max()
    shifts = step_range(args.range_ms[0] - reach, args.range_ms[1] + reach, args.step_ms)
    configured = np.array([1000 * camera["row_skew"] for camera in cameras])
    as_configured = np.mean([100 * scan(throw, cameras, shifts / 1000, configured[None] / 1000) for _, throw in used], axis=0)
    distance_as_configured = best_shift_per_skew(as_configured, shifts)[1][0]
    # 表は 0 ms を含むように 15 行ほどに間引いて表示する。
    stride = int(np.ceil(len(values) / 16))
    anchor = int(np.argmin(np.abs(values)))

    for index, side in ((LEFT, "左"), (RIGHT, "右")):
        other = "右" if index == LEFT else "左"
        skews = np.tile(configured, (len(values), 1))
        skews[:, index] = values
        tables = [100 * scan(throw, cameras, shifts / 1000, skews / 1000) for _, throw in used]

        print(
            f"\n行ごとの時刻差: {side}のカメラを動かす ({other}は {configured[1 - index]:+.1f} ms のまま)。"
            "値ごとに右のカメラの時刻のずれを合わせ直す"
        )
        print("  行ごとの時刻差(ms)  合わせ直したずれ(ms)  球ごとの距離の平均(cm)")
        pooled_shift, pooled_distance = best_shift_per_skew(np.mean(tables, axis=0), shifts)
        for k in range(anchor % stride, len(values), stride):
            print(f"  {values[k]:+14.1f}  {pooled_shift[k]:+18.1f}  {pooled_distance[k]:20.2f}")
        value, distance, at_edge = refine_minimum(values, pooled_distance)
        print(
            f"  全球をまとめた最小: {value:+.1f} ms (ずれ {np.interp(value, values, pooled_shift):+.1f} ms) で {distance:.2f} cm"
            f"{' (範囲の端)' if at_edge else ''}。設定の値 ({configured[index]:+.1f} ms) では {distance_as_configured:.2f} cm"
        )

        best, texts = [], []
        for (number, _), table in zip(used, tables):
            value, _, at_edge = refine_minimum(values, best_shift_per_skew(table, shifts)[1])
            if not at_edge:
                best.append(value)
            texts.append(f"{number}: {value:+.1f}{'*' if at_edge else ''}")
        print("  球ごとの最小 (ms。* は範囲の端で、下の集計から除く): " + "  ".join(texts))
        if best:
            print("  " + describe(best, "球"))


def quadratic_apex(times, pixels):
    """画像上の軌跡 (時刻ごとの u, v) に時刻の 2 次式を当てはめ、頂点の時刻、当てはめの残差 (px)、
    頂点での横の流れ (m/s 相当) を返す。
    """
    center = times.mean()
    tau = times - center
    design = np.stack([np.ones_like(tau), tau, tau ** 2], axis=1)
    coefficients = np.linalg.lstsq(design, pixels, rcond=None)[0]
    velocity, acceleration = coefficients[1], 2 * coefficients[2]
    # 頂点は、画像上の速度が加速度 (重力の向き) と直角になる瞬間。
    apex = -(velocity @ acceleration) / (acceleration @ acceleration)
    residual = np.sqrt(((design @ coefficients - pixels) ** 2).sum(axis=1).mean())
    # 頂点での画像上の速度を、画像上の重力加速度との比で実際の速さに直す。符号は、重力の向きを下にして
    # 画像を見たときに右へ流れるのを正とする。カメラが上下に傾いていると大きめに出るので、目安にとどめる。
    drift = velocity + acceleration * apex
    sideways = ((drift[0] * acceleration[1] - drift[1] * acceleration[0]) / (acceleration @ acceleration)
                * predict_landing.GRAVITY)
    return center + apex, residual, sideways


def apex_of_toss(times, pixels, min_frames):
    """1 台ぶんの投げ上げから頂点を求める。頂点が映っていないか、コマが足りないときは None。

    頂点の片側だけ長く映っていると、遠近による像の伸び縮みのぶん 2 次式の頂点がずれる
    (計算で作った検出では、2 台で映り方が違うと最大 5 ms ほど)。そこで頂点の前後を同じ長さに
    切りそろえて当てはめ直す。切る位置は頂点の見積もりで変わるので数回くり返す。
    """
    use = np.ones(len(times), bool)
    for _ in range(4):
        if use.sum() < max(min_frames, 3):
            return None
        with np.errstate(divide="ignore", invalid="ignore"):
            apex, residual, sideways = quadratic_apex(times[use], pixels[use])
        if not np.isfinite(apex):
            return None
        frames = int(use.sum())
        half = min(apex - times.min(), times.max() - apex)
        if half <= 0:
            return None
        use = np.abs(times - apex) <= half
    return apex, residual, sideways, frames


def report_apex(cameras, used, args):
    """投げ上げたボールの頂点の時刻を 2 台で比べる (トス試験)。"""
    print("\n回  時刻(秒)          使ったコマ数(左+右)  頂点の時刻 左, 右 (秒)  合うずれ(ms)  当てはめの残差 左, 右 (px)  横の流れ 左, 右 (m/s 相当)")
    best = []
    for number, throw in used:
        found = []
        for index, camera in enumerate(cameras):
            mine = throw["camera"] == index
            # 左の時刻の補正と、2 台の行ごとの時刻差は設定のまま足す。右の時刻の補正は足さない (これを求める)。
            times = (throw["time"][mine] + (camera["time_correction"] if index == LEFT else 0.0)
                     + camera["row_skew"] * throw["row"][mine])
            found.append(apex_of_toss(times, throw["pixel"][mine], args.min_frames))
        if None in found:
            print(f"{number:>2}  {time_span(throw)}  頂点が映っていないか、頂点の前後のコマが足りないので使いません")
            continue
        left, right = found
        # 右の時刻にこれだけ足すと、2 台の頂点の時刻がそろう。
        shift = 1000 * (left[0] - right[0])
        best.append(shift)
        frames = f"{left[3]}+{right[3]}"
        print(
            f"{number:>2}  {time_span(throw)}  {frames:>15}      {left[0]:9.4f} {right[0]:9.4f}  {shift:+12.1f}  "
            f"{left[1]:15.2f} {right[1]:5.2f}  {left[2]:+17.2f} {right[2]:+5.2f}"
        )
    if best:
        print("\n" + describe(best, "回"))
        print(f"いまの設定は {1000 * cameras[RIGHT]['time_correction']:+.1f} ms")
        print("ボールが左右に流れた回は、頂点の時刻が 2 台で逆向きにずれる。流れの小さい回だけで見ること")
    else:
        print("\n使える投げ上げがありません")


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("config", type=Path, help="設定ファイル (JSON)")
    parser.add_argument("--event", choices=["throw", "apex"], default="throw",
                        help="throw: 飛んでいるボールの視線と軌道の距離から求める (既定)。apex: 投げ上げたボールの頂点の時刻を比べる")
    parser.add_argument("--range-ms", type=float, nargs=2, default=[-30.0, 60.0], metavar=("最小", "最大"),
                        help="右のカメラの時刻のずれを探す範囲 (ms)")
    parser.add_argument("--skew-range-ms", type=float, nargs=2, default=[-30.0, 30.0], metavar=("最小", "最大"),
                        help="行ごとの時刻差を探す範囲 (ms)")
    parser.add_argument("--step-ms", type=float, default=1.0, help="探す刻み (ms)。最小の位置は刻みの間を補間して出す")
    parser.add_argument("--min-frames", type=int, default=4, help="1 台あたりのコマ数がこれより少ない球は使わない")
    parser.add_argument("--gap-s", type=float, default=0.3, help="検出がこの秒数より長く途切れたら、別の球とみなす")
    args = parser.parse_args()
    if args.min_frames < 2:
        parser.error("--min-frames は 2 以上にしてください (1 台 1 コマでは、どのずれでも視線が軌道に乗ってしまう)")
    if args.step_ms <= 0 or args.range_ms[0] >= args.range_ms[1] or args.skew_range_ms[0] >= args.skew_range_ms[1]:
        parser.error("--step-ms は正の値に、範囲は 最小 < 最大 の順にしてください")

    config = json.loads(args.config.read_text(encoding="utf-8"))
    cameras, throws = load_throws(config, args.config.parent, args.gap_s)
    for side, camera in zip(("左", "右"), cameras):
        print(
            f"{side}のカメラ: {camera['name']} (時刻の補正 {1000 * camera['time_correction']:+.1f} ms、"
            f"行ごとの時刻差 {1000 * camera['row_skew']:+.1f} ms)"
        )

    used = []
    for number, throw in enumerate(throws, 1):
        if min(throw["counts"]) >= args.min_frames:
            used.append((number, throw))
        else:
            print(f"{number:>2}  {time_span(throw)}  {frame_counts(throw):>9}      コマが足りないので使いません")
    if not used:
        sys.exit("使える球がありません (--min-frames、--gap-s と、2 台の time_offset_s を確かめてください)")

    if args.event == "apex":
        report_apex(cameras, used, args)
    else:
        report_time_shift(cameras, used, args)
        report_row_skew(cameras, used, args)


if __name__ == "__main__":
    main()
