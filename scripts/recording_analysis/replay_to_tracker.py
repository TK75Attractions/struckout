#!/usr/bin/env python3
"""録画から作った検出結果を、カメラアプリと同じ通信で ball_tracker に流し込む。

    python replay_to_tracker.py <設定.json>

先に ball_tracker を `--input network` で起動しておくこと。カメラの登録 (6060 番) と検出結果の
送信 (5050 番) を実機と同じ手順で行い、検出結果は録画どおりの間隔で送る。端末を繋がずに、
録画した素材で ball_tracker 本体の動作を確かめるためのもの。

設定で `time_sync` を true にすると、時刻合わせに対応したアプリと同じ動きをする。カメラごとに
PC とずれた時計を持たせ、その時計でのセンサー時刻を送り、ball_tracker からの時刻合わせの要求に答える。

必要なもの: Python 3、numpy。設定ファイルの書式は README.md を参照。
"""
import csv
import json
import socket
import struct
import sys
import threading
import time
import uuid
from pathlib import Path

import numpy as np

# 端末センサー座標 (x: 右、y: 上、z: 画面の手前) からカメラ座標 (x: 画像の右、y: 画像の下、
# z: 光軸) への回転。カメラアプリの defaultBackCameraRotation(90) と同じ。
DEVICE_TO_CAMERA = np.array([[0.0, -1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, -1.0]])


# --- protobuf の最小限の符号化 (api/proto/camera__ball-tracker.proto) ---

def varint(value):
    out = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        out.append(byte | (0x80 if value else 0))
        if not value:
            return bytes(out)


def field_varint(number, value):
    return varint(number << 3) + varint(value & 0xFFFFFFFFFFFFFFFF)


def field_double(number, value):
    return varint((number << 3) | 1) + struct.pack("<d", value)


def field_bytes(number, payload):
    return varint((number << 3) | 2) + varint(len(payload)) + payload


def field_float(number, value):
    return varint((number << 3) | 5) + struct.pack("<f", value)


def encode_detection(width, height, ray, image_position=None):
    body = (field_varint(1, width) + field_varint(2, height)
            + field_double(3, ray[0]) + field_double(4, ray[1]) + field_double(5, ray[2]))
    if image_position is not None:
        body += field_float(6, image_position[0]) + field_float(7, image_position[1])
    return body


def encode_detections_packet(camera_id, session_id, timestamp_ms, frame_id, detections, sensor_timestamp_ns=None):
    body = field_varint(1, camera_id) + field_bytes(2, session_id.encode()) + field_varint(3, timestamp_ms)
    body += field_varint(4, frame_id)
    for width, height, ray, image_position in detections:
        # 時刻合わせをしない送信元は、検出の位置も送らない (これまでのアプリと同じ)。
        position = image_position if sensor_timestamp_ns is not None else None
        body += field_bytes(5, encode_detection(width, height, ray, position))
    if sensor_timestamp_ns is not None:
        body += field_varint(6, sensor_timestamp_ns)
    return body


def encode_time_sync_response(sequence, receive_elapsed_ns, send_elapsed_ns, send_uptime_ns):
    response = (field_varint(1, sequence) + field_varint(2, receive_elapsed_ns)
                + field_varint(3, send_elapsed_ns) + field_varint(4, send_uptime_ns))
    return field_bytes(2, response)


def encode_camera_info(camera_id, timestamp_source, rolling_shutter_skew_ns, width, height, focal):
    info = (field_varint(1, camera_id) + field_bytes(2, b"replay") + field_bytes(3, b"replay")
            + field_varint(4, timestamp_source) + field_varint(5, rolling_shutter_skew_ns)
            + field_varint(7, width) + field_varint(8, height) + field_double(9, focal))
    return field_bytes(3, info)


def decode_varint(data, pos):
    value, shift = 0, 0
    while True:
        byte = data[pos]
        pos += 1
        value |= (byte & 0x7F) << shift
        shift += 7
        if not byte & 0x80:
            return value, pos


def decode_time_sync_request(body):
    """TcpServerPacket { time_sync_request = 2 { sequence = 1 } } の sequence を返す。ほかのパケットは None。"""
    if not body or body[0] != 0x12:
        return None
    length, pos = decode_varint(body, 1)
    inner = body[pos:pos + length]
    if not inner:
        return 0
    if inner[0] != 0x08:
        return None
    return decode_varint(inner, 1)[0]


class DeviceClock:
    """PC とずれていて、進み方も少し違う、端末の時計。

    elapsedRealtimeNanos() にあたる値を返す。センサー時刻は、設定の timestamp_source が "unknown" の
    ときは System.nanoTime() にあたる時計 (端末が眠っていた時間だけ遅れている) で付ける。
    """

    def __init__(self, camera, perf_anchor):
        self.perf_anchor = perf_anchor
        self.base_s = 1000.0 + float(camera.get("clock_offset_s", 0.0))
        self.rate = 1.0 + float(camera.get("clock_drift_ppm", 0.0)) * 1e-6
        self.uptime_lag_ns = int(round(float(camera.get("uptime_lag_s", 0.0)) * 1e9))
        self.realtime = camera.get("timestamp_source", "realtime") == "realtime"

    def elapsed_ns(self, perf_time=None):
        perf_time = time.perf_counter() if perf_time is None else perf_time
        return int(round((self.base_s + (perf_time - self.perf_anchor) * self.rate) * 1e9))

    def uptime_ns(self, perf_time=None):
        return self.elapsed_ns(perf_time) - self.uptime_lag_ns

    def sensor_ns(self, perf_time):
        return self.elapsed_ns(perf_time) if self.realtime else self.uptime_ns(perf_time)


def answer_time_sync(sock, clock, camera_info):
    """カメラの設定を知らせ、時刻合わせの要求に答え続ける。接続が切れたら終わる。"""
    try:
        send_packet(sock, camera_info)
        while True:
            header = sock.recv(4, socket.MSG_WAITALL)
            if len(header) < 4:
                return
            received_ns = clock.elapsed_ns()
            (length,) = struct.unpack("<I", header)
            body = sock.recv(length, socket.MSG_WAITALL) if length else b""
            sequence = decode_time_sync_request(body)
            if sequence is None:
                continue
            perf_now = time.perf_counter()
            send_packet(sock, encode_time_sync_response(
                sequence, received_ns, clock.elapsed_ns(perf_now), clock.uptime_ns(perf_now)))
    except OSError:
        return


def encode_camera_pose(camera_id, position, rotation_deg):
    pose = b"".join(field_double(i + 1, v) for i, v in enumerate([*position, *rotation_deg]))
    update = field_varint(1, camera_id) + field_bytes(2, pose)
    return field_bytes(1, update)


def send_packet(sock, body):
    sock.sendall(struct.pack("<I", len(body)) + body)


def receive_camera_id(sock):
    """TcpServerPacket { camera_id = 1 } を読む。id が 0 のときは本文が空で届く。"""
    (length,) = struct.unpack("<I", sock.recv(4, socket.MSG_WAITALL))
    body = sock.recv(length, socket.MSG_WAITALL) if length else b""
    if not body:
        return 0
    assert body[0] == 0x08, body
    value, shift = 0, 0
    for byte in body[1:]:
        value |= (byte & 0x7F) << shift
        shift += 7
        if not byte & 0x80:
            break
    return value


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    config_path = Path(sys.argv[1])
    config = json.loads(config_path.read_text(encoding="utf-8"))
    base = config_path.parent
    width, height = config["image_size"]
    host = config.get("host", "127.0.0.1")
    start, end = config.get("time_range_s", [float("-inf"), float("inf")])
    min_size = config.get("min_bbox_px", 0)
    time_sync = bool(config.get("time_sync", False))
    # 待っている間にほかのスレッド (時刻合わせの返事) が動けるよう、切り替えを細かくする。
    sys.setswitchinterval(0.0005)
    perf_anchor = time.perf_counter()

    # 検出結果を、送る時刻の順に並べる。
    frames = []
    cameras = []
    for index, camera in enumerate(config["cameras"]):
        focal = float(camera.get("focal_px", config.get("focal_px")))
        by_frame = {}
        with (base / camera["detections"]).open(encoding="utf-8") as handle:
            for row in csv.DictReader(handle):
                # 全カメラ共通の時計 (基準の動画の先頭からの秒) に直す
                t = float(row["time_s"]) - camera["time_offset_s"]
                w, h = int(row["width"]), int(row["height"])
                if not (start <= t <= end) or min(w, h) < min_size:
                    continue
                in_camera = np.array([
                    (int(row["x"]) + w / 2 - width / 2) / focal,
                    (int(row["y"]) + h / 2 - height / 2) / focal,
                    1.0,
                ])
                # アプリは端末センサー座標の視線を送り、ball_tracker が Camera Pose で世界座標に回す。
                image_position = ((int(row["x"]) + w / 2) / width, (int(row["y"]) + h / 2) / height)
                by_frame.setdefault((t, int(row["frame"])), []).append(
                    (w, h, DEVICE_TO_CAMERA.T @ in_camera, image_position))
        for (t, frame_id), detections in by_frame.items():
            frames.append((t, index, frame_id, detections))
        cameras.append(camera)
    frames.sort(key=lambda f: f[0])
    if not frames:
        sys.exit("送る検出結果がありません")

    # カメラの登録。ball_tracker は登録済みの台数を次の id にするので、1 台ずつ順に行う。
    control, data = [], []
    for index, camera in enumerate(cameras):
        sock = socket.create_connection((host, 6060))
        camera_id = receive_camera_id(sock)
        send_packet(sock, encode_camera_pose(camera_id, camera["position_m"], camera["rotation_deg"]))
        control.append(sock)
        camera["id"] = camera_id
        print(f"{camera['name']}: camera id {camera_id}")
        if time_sync:
            sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
            camera["clock"] = DeviceClock(camera, perf_anchor)
            info = encode_camera_info(
                camera_id, 2 if camera["clock"].realtime else 1,
                int(round(float(camera.get("rolling_shutter_skew_ms", 0.0)) * 1e6)),
                width, height, float(camera.get("focal_px", config.get("focal_px"))))
            threading.Thread(target=answer_time_sync, args=(sock, camera["clock"], info), daemon=True).start()
        time.sleep(0.3)
    for camera in cameras:
        sock = socket.create_connection((host, 5050))
        sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        data.append(sock)
        camera["session"] = str(uuid.uuid4())
    # 時刻合わせの最初の何往復かが済むのを待つ。
    time.sleep(1.0 if time_sync else 0.5)

    # 録画と同じ間隔で送る。time.sleep は Windows では 15ms 単位でしか効かないので、
    # 直前まで寝たあとは時計を見ながら待つ。
    first = frames[0][0]
    wall_start = time.time() + 0.5
    perf_start = time.perf_counter() + 0.5
    epoch_ms = wall_start * 1000.0
    for t, index, frame_id, detections in frames:
        due = perf_start + (t - first)
        remaining = due - time.perf_counter()
        if remaining > 0.02:
            time.sleep(remaining - 0.02)
        while time.perf_counter() < due:
            pass
        camera = cameras[index]
        timestamp_ms = int(round(epoch_ms + (t - first) * 1000.0))
        sensor_timestamp_ns = camera["clock"].sensor_ns(due) if time_sync else None
        send_packet(data[index], encode_detections_packet(
            camera["id"], camera["session"], timestamp_ms, frame_id, detections, sensor_timestamp_ns))
    print(f"{len(frames)} フレームを送りました。基準の時計の {first:.3f} 秒が {epoch_ms / 1000.0:.3f} (UNIX 秒) にあたります")
    # 最後の着弾の予測が報告されるまで接続を保つ
    time.sleep(1.5)
    for sock in control + data:
        sock.close()


if __name__ == "__main__":
    main()
