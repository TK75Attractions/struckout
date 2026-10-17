#!/usr/bin/env python3
"""録画を、カメラアプリと同じ手順で ball_tracker に動画として流す (PC で解析するときの確認用)。

    python stream_video_to_tracker.py <設定.json>

先に ball_tracker を起動しておく (5052 番で動画を受け付ける)。カメラごとに、録画の指定した範囲を H.264
(B フレームなし) にしてから、カメラの登録 (6060 番) と動画の送信 (5052 番) を、アプリと同じ手順で行う。
フレームは録画どおりの間隔で送る。時刻合わせには答えない (これまでのアプリと同じ扱いになる)。

設定ファイル (JSON) の項目。ファイルの場所は設定ファイルからの相対パスで書く。

| 項目 | 内容 |
|---|---|
| `time_range_s` | 送る範囲 (全カメラ共通の時計での秒) |
| `video_size` | 送る動画の大きさ。既定は [1280, 720] |
| `analysis_size` | PC が解析する画像の大きさ。既定は [640, 360] |
| `tracker` | 解析の設定 (`min_color_shift`、`foreground_persistence_frames` など)。省略するとアプリの既定値 |
| `cameras[].video` | 録画 (縦持ちの印は無視して、センサーの向きのまま使う) |
| `cameras[].time_offset_s` | 共通の時計の 0 秒が、この録画の何秒にあたるか |
| `cameras[].focal_px` | 解析画像での焦点距離 (px) |
| `cameras[].position_m`、`rotation_deg` | `replay_to_tracker.py` と同じ |

必要なもの: Python 3、PATH 上の ffmpeg と ffprobe (または環境変数 FFMPEG、FFPROBE)。
"""
import json
import os
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time
import uuid
from pathlib import Path

from replay_to_tracker import (encode_camera_pose, field_bytes, field_double, field_varint, receive_camera_id,
                               send_packet)

FFMPEG = os.environ.get("FFMPEG", "ffmpeg")
FFPROBE = os.environ.get("FFPROBE", "ffprobe")
# アプリの既定の背面カメラの向き (センサーの向き 90 度)。defaultBackCameraRotation(90)
DEVICE_TO_CAMERA = [0.0, -1.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0]
TRACKER_DEFAULT = {"accumulate_weight": 0.5, "binary_threshold": 15.0, "min_contour_area": 80.0,
                   "foreground_persistence_frames": 60, "min_color_shift": 10.0}


def encode_config(camera_id, session_id, video_size, frame_rate, analysis_size, focal, tracker):
    aw, ah = analysis_size
    body = field_varint(1, camera_id) + field_bytes(2, session_id.encode()) + field_bytes(3, b"video/avc")
    body += field_varint(4, video_size[0]) + field_varint(5, video_size[1]) + field_double(6, frame_rate)
    body += field_varint(7, aw) + field_varint(8, ah)
    body += field_double(9, focal) + field_double(10, focal) + field_double(11, aw / 2) + field_double(12, ah / 2)
    body += field_bytes(13, b"".join(struct.pack("<d", v) for v in DEVICE_TO_CAMERA))
    # 14〜17 は検出を受け付ける範囲 (すべて 0 なら画像の全体)
    body += field_double(18, tracker["accumulate_weight"]) + field_double(19, tracker["binary_threshold"])
    body += field_double(20, tracker["min_contour_area"]) + field_varint(21, tracker["foreground_persistence_frames"])
    body += field_double(22, tracker["min_color_shift"])
    return field_bytes(1, body)


def encode_frame(data, codec_config=False, key_frame=False, sensor_timestamp_ns=0, timestamp_ms=0):
    body = field_bytes(1, data)
    if codec_config:
        body += field_varint(2, 1)
    if key_frame:
        body += field_varint(3, 1)
    if sensor_timestamp_ns:
        body += field_varint(4, sensor_timestamp_ns)
    if timestamp_ms:
        body += field_varint(5, timestamp_ms)
    return field_bytes(2, body)


def nal_units(stream):
    """Annex B の H.264 を、開始符号つきの NAL ユニットに分ける。(種類, バイト列) を返す。"""
    starts = []
    i = 0
    while True:
        i = stream.find(b"\x00\x00\x01", i)
        if i < 0:
            break
        begin = i - 1 if i > 0 and stream[i - 1] == 0 else i
        starts.append((begin, i + 3))
        i += 3
    units = []
    for n, (begin, payload) in enumerate(starts):
        end = starts[n + 1][0] if n + 1 < len(starts) else len(stream)
        units.append((stream[payload] & 0x1F, stream[begin:end]))
    return units


def access_units(stream):
    """区切り (AUD) ごとにまとめたフレームと、最初の SPS/PPS を返す。"""
    config = b""
    frames = []
    current, key = b"", False
    for kind, unit in nal_units(stream):
        if kind == 9 and current:
            frames.append((current, key))
            current, key = b"", False
        if kind in (7, 8) and len(config) < 512 and not frames:
            config += unit
        current += unit
        key = key or kind == 5
    if current:
        frames.append((current, key))
    return config, frames


def prepare(camera, base, start, end, video_size):
    """録画の範囲を H.264 にし、フレームと、そのフレームの録画での時刻 (秒) を返す。"""
    video = base / camera["video"]
    begin = start + camera["time_offset_s"]
    duration = end - start
    # 行末に区切りの "," が付くことがある
    lines = subprocess.run(
        [FFPROBE, "-v", "error", "-select_streams", "v:0", "-read_intervals", f"{begin}%+{duration}",
         "-show_entries", "frame=pts_time", "-of", "csv=p=0", str(video)],
        capture_output=True, text=True, check=True).stdout.split()
    times = [float(line.split(",")[0]) for line in lines if line.split(",")[0]]
    times = [t for t in times if begin <= t < begin + duration]
    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp) / "video.h264"
        subprocess.run(
            [FFMPEG, "-v", "error", "-y", "-display_rotation", "0", "-i", str(video), "-ss", f"{begin}", "-t", f"{duration}",
             "-vf", f"scale={video_size[0]}:{video_size[1]}", "-fps_mode", "passthrough", "-c:v", "libx264",
             "-preset", "veryfast", "-tune", "zerolatency", "-profile:v", "baseline", "-bf", "0", "-g", "60",
             "-b:v", "20M", "-x264-params", "aud=1", "-pix_fmt", "yuv420p", "-f", "h264", str(out)],
            check=True)
        config, frames = access_units(out.read_bytes())
    if len(times) != len(frames):
        print(f"{camera['name']}: 録画のフレーム {len(times)} 枚と、H.264 のフレーム {len(frames)} 枚が合いません。"
              "等間隔とみなします", file=sys.stderr)
        rate = len(frames) / duration
        times = [begin + i / rate for i in range(len(frames))]
    return config, list(zip(times, frames)), len(frames) / duration


def run_camera(camera, host, config, frames, frame_rate, args, epoch):
    session_id = str(uuid.uuid4())
    control = socket.create_connection((host, 6060))
    camera_id = receive_camera_id(control)
    send_packet(control, encode_camera_pose(camera_id, camera["position_m"], camera["rotation_deg"]))
    video = socket.create_connection((host, 5052))
    video.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
    (length,) = struct.unpack("<I", video.recv(4, socket.MSG_WAITALL))
    answer = video.recv(length, socket.MSG_WAITALL) if length else b""
    if answer != b"\x08\x01":
        print(f"{camera['name']}: PC が解析を引き受けませんでした ({answer!r})", file=sys.stderr)
        return
    send_packet(video, encode_config(camera_id, session_id, args["video_size"], frame_rate, args["analysis_size"],
                                     camera["focal_px"], args["tracker"]))
    send_packet(video, encode_frame(config, codec_config=True))
    print(f"{camera['name']}: camera id {camera_id}、{len(frames)} フレーム")
    offset = camera["time_offset_s"]
    for video_time, (data, key) in frames:
        # 共通の時計での時刻に直し、その時刻に送る
        send_at = epoch + (video_time - offset - args["start"])
        delay = send_at - time.time()
        if delay > 0:
            time.sleep(delay)
        send_packet(video, encode_frame(data, key_frame=key, timestamp_ms=int(send_at * 1000)))
    time.sleep(1.0)
    video.close()
    control.close()


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    config_path = Path(sys.argv[1])
    config = json.loads(config_path.read_text(encoding="utf-8"))
    base = config_path.parent
    start, end = config["time_range_s"]
    args = {
        "start": start,
        "video_size": config.get("video_size", [1280, 720]),
        "analysis_size": config.get("analysis_size", [640, 360]),
        "tracker": {**TRACKER_DEFAULT, **config.get("tracker", {})},
    }
    prepared = [prepare(camera, base, start, end, args["video_size"]) for camera in config["cameras"]]
    epoch = time.time() + 2.0
    threads = []
    for camera, (codec_config, frames, rate) in zip(config["cameras"], prepared):
        thread = threading.Thread(target=run_camera,
                                  args=(camera, config.get("host", "127.0.0.1"), codec_config, frames, rate, args, epoch))
        thread.start()
        threads.append(thread)
        time.sleep(0.5)  # ball_tracker は接続した順に番号を振るので、順番をそろえる
    for thread in threads:
        thread.join()
    print(f"送り終えました。基準の時計の {start:.3f} 秒が {epoch:.3f} (UNIX 秒) にあたります")


if __name__ == "__main__":
    main()
