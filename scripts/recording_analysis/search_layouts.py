#!/usr/bin/env python3
"""2 台のカメラの置き方を総当たりし、着弾位置の予測がよく当たる順に並べる。

    python search_layouts.py

simulate_layouts.py と同じシミュレーションを、壁・床・天井・奥面に置ける位置と向きの組み合わせに
対して回す。2 台は左右対称に置くか、同じ側の壁の上下に置く。120fps (16:9) を前提にしている。
必要なもの: Python 3、numpy。
"""
import itertools
import multiprocessing

import numpy as np

import simulate_layouts as s

FPS, EXPOSURE, ASPECT = 120.0, 1 / 120, "16:9"
MIDDLE = (s.Y_MIN + s.Y_MAX) / 2
WALL = s.Y_MAX - 0.02
FLOOR, CEILING = s.Z_FLOOR + 0.04, s.Z_CEILING - 0.03
BACK = s.PLANE_X + 0.05


def candidates():
    """(説明, 1 台目の姿勢) を列挙する。姿勢は x, y, z, yaw, pitch, roll。"""
    out = []
    # 側面の壁、または壁と床・天井の角。反対側の壁の中ほどを狙い、そこから上下左右に振る。
    for x, z in itertools.product((0.10, 0.30, 0.55, 0.80), (FLOOR, 1.45, 1.80, 2.15, CEILING)):
        aim_yaw = np.degrees(np.arctan2(0.50 - x, WALL - s.Y_MIN))
        aim_pitch = np.degrees(np.arctan2(1.80 - z, WALL - s.Y_MIN))
        for dyaw, dpitch, roll in itertools.product((-12, 0, 12), (-12, 0, 12), (0, 90)):
            out.append(("壁", (x, WALL, z, aim_yaw + dyaw, aim_pitch + dpitch, roll)))
    # 床や天井の、壁から離れた位置
    for x, y, z in itertools.product((0.30, 0.55, 0.80), (0.75, 0.95), (FLOOR, CEILING)):
        up = 1 if z == FLOOR else -1
        for yaw, pitch, roll in itertools.product((-15, 10), (up * 55, up * 75), (0, 90)):
            out.append(("床" if up == 1 else "天井", (x, y, z, yaw, pitch, roll)))
    # 奥面の、角から離れた位置
    for y, z in itertools.product((0.80, 1.00), (1.45, 1.80, 2.15)):
        for yaw, pitch, roll in itertools.product((20, 35), (-15, 0, 15), (0, 90)):
            out.append(("奥面", (BACK, y, z, yaw, pitch, roll)))
    return out


def pairs(pose):
    x, y, z, yaw, pitch, roll = pose
    yield "左右対称", s.mirrored_pair(x, y, z, yaw, pitch, roll)
    if y == WALL and z in (FLOOR, CEILING):
        # 同じ側の壁の上下の角。もう 1 台は高さと上下の向きを反転する。
        other_z = CEILING if z == FLOOR else FLOOR
        yield "同じ壁の上下", [s.posed_camera(x, y, z, yaw, pitch, roll), s.posed_camera(x, y, other_z, yaw, -pitch, -roll)]


def evaluate(job):
    surface, pose, kind, trials, seed = job
    cameras = dict(pairs(pose))[kind]
    result = s.simulate(cameras, FPS, EXPOSURE, ASPECT, np.random.default_rng(seed), trials=trials)
    return surface, pose, kind, result


def describe(surface, pose, kind, r):
    x, y, z, yaw, pitch, roll = pose
    hold = "縦持ち" if roll == 0 else "横持ち"
    return (
        f"{surface:2s} {kind:6s} 奥面から {100 * (x - s.PLANE_X):3.0f}cm 壁から {100 * (s.Y_MAX - y):3.0f}cm 床から {100 * (z - s.Z_FLOOR):3.0f}cm  "
        f"振り {yaw:5.1f}° 上下 {pitch:+6.1f}° {hold}  予測 {r['predicted']:4.0%}  10cm 以内 {r['within_10']:4.0%}  "
        f"誤差 y {r['rms_y']:4.1f} z {r['rms_z']:4.1f} cm"
    )


def main():
    jobs = [(surface, pose, kind, 600, 11) for surface, pose in candidates() for kind, _ in pairs(pose)]
    print(f"{len(jobs)} 通りを調べます")
    with multiprocessing.Pool() as pool:
        coarse = pool.map(evaluate, jobs, chunksize=4)
        coarse.sort(key=lambda t: -t[3]["within_10"])
        # 上位は試行回数を増やし、別の乱数で確かめ直す。
        finalists = [(surface, pose, kind, 4000, 12) for surface, pose, kind, _ in coarse[:12]]
        final = pool.map(evaluate, finalists, chunksize=1)
    final.sort(key=lambda t: -t[3]["within_10"])
    print("\n上位 (4000 球で確かめ直した値)")
    for item in final:
        print(describe(*item))
    print("\n置く面ごとの最良 (600 球)")
    seen = set()
    for surface, pose, kind, r in coarse:
        if (surface, kind) not in seen:
            seen.add((surface, kind))
            print(describe(surface, pose, kind, r))


if __name__ == "__main__":
    main()
