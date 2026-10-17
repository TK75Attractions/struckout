#!/usr/bin/env python3
"""カメラアプリの検出処理 (ObjectTracker.kt) を、録画した動画に対して PC 上で再現する。

    python detect_objects.py <動画> --start 380 --duration 5 --out detections.csv

端末を繋がずに、同じ動画で検出の設定を変えて比べるためのもの。処理の手順と既定値は
struckoutCameraApp の ObjectTracker と CameraRepository に合わせてあるので、
アプリ側を変えたらこちらも合わせること。

検出した領域のうち、背景からの色みの変化 (color_shift) が --min-color-shift (既定 10) に届かないものは、
影や明るさだけの変化とみなして捨てる。0 を指定すると色みでは絞り込まない。

出力する CSV の列は frame, time_s, x, y, width, height, color_shift, saturation。
saturation は以前の判定 (平均色の彩度が 0.25 以上) と比べるための値で、絞り込みには使わない。

必要なもの: Python 3、numpy、opencv-python、PATH 上の ffmpeg。
"""
import argparse
import csv
import subprocess
from pathlib import Path

import cv2
import numpy as np


class ObjectTracker:
    """ObjectTracker.kt と同じ手順で、背景との差分から動く物体を見つける。

    min_color_shift は、検出した領域の平均色が、背景の同じ場所の平均色から色みをどれだけ変えたかの
    下限 (8 ビットの階調)。影や照明のような明るさだけの変化を捨てるのに使う。
    0 のときは色みで絞り込まない。
    """

    def __init__(self, accumulate_weight=0.5, binary_threshold=15.0, min_contour_area=80.0,
                 foreground_persistence_frames=15, min_color_shift=10.0):
        self.accumulate_weight = accumulate_weight
        self.binary_threshold = binary_threshold
        self.min_contour_area = min_contour_area
        self.foreground_persistence_frames = foreground_persistence_frames
        self.min_color_shift = min_color_shift
        self.background = None
        self.foreground_age = None
        self.kernel = cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (5, 5))

    def next_frame(self, frame):
        """検出した物体の (x, y, 幅, 高さ, 色みの変化, 彩度) を返す。"""
        if self.background is None:
            self.background = frame.astype(np.float32)
            self.foreground_age = np.zeros(frame.shape[:2], np.uint16)
            return []

        background8 = cv2.convertScaleAbs(self.background)
        delta = cv2.absdiff(frame, background8).max(axis=2)
        _, mask = cv2.threshold(delta, self.binary_threshold, 255, cv2.THRESH_BINARY)
        mask = cv2.morphologyEx(mask, cv2.MORPH_OPEN, self.kernel, iterations=1,
                                borderType=cv2.BORDER_CONSTANT)
        mask = cv2.morphologyEx(mask, cv2.MORPH_CLOSE, self.kernel, iterations=2,
                                borderType=cv2.BORDER_CONSTANT)

        foreground = mask > 0
        self.foreground_age[~foreground] = 0
        self.foreground_age[foreground] += 1
        update = (~foreground) | (self.foreground_age >= self.foreground_persistence_frames)
        cv2.accumulateWeighted(frame, self.background, self.accumulate_weight, update.astype(np.uint8))

        contours, _ = cv2.findContours(mask, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_SIMPLE)
        found = []
        for contour in contours:
            if cv2.contourArea(contour) < self.min_contour_area:
                continue
            x, y, w, h = cv2.boundingRect(contour)
            inside = np.zeros((h, w), np.uint8)
            cv2.drawContours(inside, [contour], 0, 255, cv2.FILLED, cv2.LINE_8, offset=(-x, -y))
            now = np.array(cv2.mean(frame[y:y + h, x:x + w], inside)[:3])
            # 比べる相手は、このコマの差分に使った背景 (上で更新する前のもの)。
            before = np.array(cv2.mean(background8[y:y + h, x:x + w], inside)[:3])
            color_shift = self._color_shift(now, before)
            if self.min_color_shift > 0.0 and color_shift < self.min_color_shift:
                continue
            found.append((x, y, w, h, color_shift, self._saturation(now)))
        return found

    @staticmethod
    def _color_shift(now, before):
        """輪郭の内側の平均色が before から now へ、色みをどれだけ変えたかを返す (8 ビットの階調)。

        次の 2 つの残りの小さいほう。残りの大きさは、3 チャンネルをベクトルとみたときの長さ。
        (1) 明るさだけが何倍かになったとみなしたときの残り。影や照明の変化では、面の色に関係なく小さい。
            ただし 0 にはならず、録画では変化の大きさの 1〜3 割が残った (既知の影で 5.5〜7.4)。
            下限を下げるときは、影が通らないかを確かめること。
        (2) 全チャンネルが同じ量だけ増減したとみなしたときの残り。黒い背景に白い物体のような、
            色のないものどうしの変化ではほぼ 0。
        """
        denominator = before @ before
        k = now @ before / denominator if denominator > 0 else 0.0
        scale_residual = np.linalg.norm(now - k * before)
        delta = now - before
        offset_residual = np.linalg.norm(delta - delta.mean())
        return float(min(scale_residual, offset_residual))

    @staticmethod
    def _saturation(mean):
        """平均色の彩度 (HSV の S と同じ定義)。以前の判定と比べるための値で、絞り込みには使わない。"""
        return 0.0 if mean.max() <= 0 else float((mean.max() - mean.min()) / mean.max())


def read_frame_times(video):
    """各コマの表示時刻 (秒) を、表示順に並べて返す。"""
    cmd = [
        "ffprobe", "-v", "error", "-select_streams", "v:0",
        "-show_entries", "packet=pts_time", "-of", "csv=p=0", str(video),
    ]
    out = subprocess.run(cmd, stdout=subprocess.PIPE, check=True, text=True).stdout
    return np.sort(np.array([float(line.strip(",")) for line in out.split() if line.strip(",")]))


def read_frames(video, times, first_frame, count, size):
    """first_frame から count コマを、センサーの向きのまま size に縮めた画像として順に返す。

    コマ番号は表示順で数える。-ss で頭出しすると、動画によっては 1〜2 コマずれて始まる
    (B フレームのある iPhone の HEVC で確認) ので、先頭から復号して番号で選ぶ。
    """
    width, height = size
    last_frame = first_frame + count - 1
    # 縦持ちで撮った動画には「90 度回して表示する」印が付いていて、ffmpeg は既定でそのとおりに回す。
    # 回されるとセンサーの向きでなくなるので、印を 0 度で上書きする。-noautorotate は ffmpeg 9 では
    # 効かなかった。
    cmd = [
        "ffmpeg", "-v", "fatal", "-display_rotation", "0", "-i", str(video),
        "-map", "0:v:0", "-an", "-fps_mode", "passthrough",
        "-vf", f"select='between(n\\,{first_frame}\\,{last_frame})',scale={width}:{height}:flags=area",
        "-frames:v", str(count), "-f", "rawvideo", "-pix_fmt", "rgb24", "-",
    ]
    process = subprocess.Popen(cmd, stdout=subprocess.PIPE)
    frame_bytes = width * height * 3
    while True:
        raw = process.stdout.read(frame_bytes)
        if len(raw) < frame_bytes:
            break
        yield np.frombuffer(raw, np.uint8).reshape(height, width, 3)
    process.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("video", type=Path)
    parser.add_argument("--start", type=float, default=0.0, help="処理を始める時刻 (秒)")
    parser.add_argument("--duration", type=float, help="処理する長さ (秒)。省略すると最後まで")
    parser.add_argument("--size", default="640x360", help="解析する画像の大きさ (センサーの向き)")
    parser.add_argument("--min-color-shift", type=float, default=10.0,
                        help="背景からの色みの変化の下限 (8 ビットの階調)。0 にすると色みで絞り込まない")
    parser.add_argument("--persistence", type=int, default=15,
                        help="止まった物体を背景とみなすまでのコマ数 (30fps で 15)")
    parser.add_argument("--ignored-margins", default="0,0,0,0",
                        help="アプリの Detection Area と同じ、プレビューの左,上,右,下から無視する幅 (%%)")
    parser.add_argument("--out", type=Path, default=Path("detections.csv"))
    args = parser.parse_args()

    size = tuple(int(v) for v in args.size.split("x"))
    # アプリの DetectionArea と同じ対応。プレビューは解析画像を時計回りに 90 度回したものなので、
    # プレビューの上下が解析画像の左右に、プレビューの右左が解析画像の上下にあたる。
    left, top, right, bottom = (float(v) / 100 for v in args.ignored_margins.split(","))
    x_range = (size[0] * top, size[0] * (1 - bottom))
    y_range = (size[1] * right, size[1] * (1 - left))
    times = read_frame_times(args.video)
    first = int(np.searchsorted(times, args.start))
    last = len(times) if args.duration is None else int(np.searchsorted(times, args.start + args.duration))
    tracker = ObjectTracker(foreground_persistence_frames=args.persistence,
                            min_color_shift=args.min_color_shift)

    rows = 0
    with args.out.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.writer(handle)
        writer.writerow(["frame", "time_s", "x", "y", "width", "height", "color_shift", "saturation"])
        for offset, frame in enumerate(read_frames(args.video, times, first, last - first, size)):
            index = first + offset
            for x, y, w, h, color_shift, saturation in tracker.next_frame(frame):
                center = (x + w / 2, y + h / 2)
                if not (x_range[0] <= center[0] <= x_range[1] and y_range[0] <= center[1] <= y_range[1]):
                    continue
                writer.writerow([index, f"{times[index]:.4f}", x, y, w, h,
                                 f"{color_shift:.3f}", f"{saturation:.3f}"])
                rows += 1
    print(f"コマ {first}-{last - 1} から {rows} 件の検出を {args.out} に書き出しました")


if __name__ == "__main__":
    main()
