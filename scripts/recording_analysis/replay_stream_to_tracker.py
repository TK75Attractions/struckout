#!/usr/bin/env python3
"""ball_tracker の録画 (.vsp) を、スマホが送ったときと同じ間隔で流し直す。

    python replay_stream_to_tracker.py <記録フォルダー> --poses <poses.json> [--from 12:29:30] [--seconds 60] [--repeat 1]

カメラの登録 (6060 番) と動画の送信 (5052 番) を行う。送るのは実際に受け取ったパケットそのもの
(設定の camera_id だけ、新しく振られた番号に置換)。受信時刻の間隔で流し、PC 内の解析遅延を測る。
時刻合わせには答えないので、ball_tracker は受信時刻から時計を見積もる。

先に ball_tracker を --input network で起動しておく。poses.json は replay_video と同じ形。
録画全体はメモリに載せず、再生位置と設定のオフセットだけを覚えて 1 パケットずつ読む。
--repeat は同じ区間に毎回接続し直す試験であり、実機の途切れない動画の再現とは異なる。
"""

from __future__ import annotations

import argparse
import concurrent.futures
import contextlib
import json
import math
import socket
import struct
import sys
import threading
import time
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import BinaryIO, Iterator

from replay_to_tracker import encode_camera_pose, field_varint, send_packet

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from struckout_common import server_port, setup_console


def read_varint(data: bytes, pos: int) -> tuple[int, int]:
    value = shift = 0
    while pos < len(data) and shift < 70:
        byte = data[pos]
        pos += 1
        value |= (byte & 0x7F) << shift
        if byte < 0x80:
            return value, pos
        shift += 7
    raise ValueError("Protobuf の varint が不正です")


def fields(data: bytes) -> Iterator[tuple[int, int, bytes, int | bytes]]:
    """Protobuf の最上位の (番号, 型, キーを含むバイト列, 値) を返す。"""
    pos = 0
    while pos < len(data):
        start = pos
        key, pos = read_varint(data, pos)
        number, wire = key >> 3, key & 7
        if wire == 0:
            value, pos = read_varint(data, pos)
        elif wire in (1, 2, 5):
            if wire == 2:
                length, pos = read_varint(data, pos)
            else:
                length = 8 if wire == 1 else 4
            if pos + length > len(data):
                raise ValueError("Protobuf パケットが途中で切れています")
            value = data[pos:pos + length]
            pos += length
        else:
            raise ValueError(f"未対応の Protobuf wire type: {wire}")
        yield number, wire, data[start:pos], value


def with_camera_id(packet: bytes, camera_id: int) -> bytes:
    """VideoStreamPacket.config.camera_id を書き換える。設定でなければそのまま。"""
    # 動画の payload をコピー・解釈する必要はない。
    if not packet or packet[0] != 0x0A:
        return packet
    top = list(fields(packet))
    config_fields = list(fields(top[0][3]))
    config = b"".join(field_varint(1, camera_id) if number == 1 else raw
                      for number, _, raw, _ in config_fields)
    if not any(number == 1 for number, *_ in config_fields):
        config = field_varint(1, camera_id) + config
    return b"\x0a" + encode_length(len(config)) + config


def encode_length(value: int) -> bytes:
    out = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        out.append(byte | (0x80 if value else 0))
        if not value:
            return bytes(out)


def read_packet(source: BinaryIO, file_size: int) -> tuple[int, bytes] | None:
    header = source.read(12)
    if not header:
        return None
    if len(header) != 12:
        raise ValueError(".vsp のヘッダーが途中で切れています")
    received, length = struct.unpack("<qI", header)
    # 壊れた長さを受けて巨大な領域を割り当てない。
    if length > file_size - source.tell():
        raise ValueError(".vsp のパケット長がファイルの残りを超えています")
    body = source.read(length)
    if len(body) != length:
        raise ValueError(".vsp のパケットが途中で切れています")
    return received, body


@dataclass(frozen=True)
class RecordingSlice:
    path: Path
    offset: int
    first_ns: int
    end_ns: int
    head_offsets: tuple[int, ...]

    def packets(self) -> Iterator[tuple[int, bytes]]:
        with self.path.open("rb") as source:
            file_size = self.path.stat().st_size
            for offset in self.head_offsets:
                source.seek(offset)
                item = read_packet(source, file_size)
                if item is None:
                    raise ValueError(f"設定パケットがありません: {self.path}")
                yield self.first_ns, item[1]
            source.seek(self.offset)
            while (item := read_packet(source, file_size)) is not None:
                if item[0] > self.end_ns:
                    return
                yield item


def index_vsp(path: Path, start_ns: int, end_ns: int) -> RecordingSlice:
    """先頭まで逐次走査し、範囲の前の config/SPS/PPS の位置だけを残す。"""
    head = []
    with path.open("rb") as source:
        file_size = path.stat().st_size
        while True:
            offset = source.tell()
            item = read_packet(source, file_size)
            if item is None or item[0] > end_ns:
                raise ValueError(f"指定区間にパケットがありません: {path}")
            received, body = item
            if received >= start_ns:
                return RecordingSlice(path, offset, received, end_ns, tuple(head))
            top = list(fields(body))
            is_config = top and top[0][0] == 1
            is_codec = top and top[0][0] == 2 and any(n == 2 and value for n, _, _, value in fields(top[0][3]))
            if is_config or is_codec:
                head.append(offset)


def recv_exact(connection: socket.socket, length: int) -> bytes:
    """MSG_WAITALL に依存せず、短い TCP read を最後まで読む。"""
    result = bytearray()
    while len(result) < length:
        part = connection.recv(length - len(result))
        if not part:
            raise ConnectionError("ball_tracker が応答の途中で接続を閉じました")
        result.extend(part)
    return bytes(result)


def receive_camera_id(connection: socket.socket) -> int:
    (length,) = struct.unpack("<I", recv_exact(connection, 4))
    body = recv_exact(connection, length)
    if not body:
        return 0
    for number, wire, _, value in fields(body):
        if number == 1 and wire == 0:
            return value
    raise ValueError("ball_tracker から camera_id が返されませんでした")


def open_stream(stack: contextlib.ExitStack, host: str, pose: dict) -> tuple[socket.socket, int]:
    control = stack.enter_context(socket.create_connection((host, server_port("ball-tracker", "camera-ctrl")), timeout=15))
    camera_id = receive_camera_id(control)
    send_packet(control, encode_camera_pose(camera_id, pose["position_m"], pose["rotation_deg"]))
    video = stack.enter_context(socket.create_connection((host, server_port("ball-tracker", "camera-video")), timeout=15))
    video.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
    (length,) = struct.unpack("<I", recv_exact(video, 4))
    answer = recv_exact(video, length)
    if answer != b"\x08\x01":
        raise ValueError(f"PC が解析を引き受けませんでした: {answer!r}")
    return video, camera_id


def stream(recording: RecordingSlice, video: socket.socket, camera_id: int,
           epoch: float, stop: threading.Event) -> None:
    sent = 0
    for received, body in recording.packets():
        if stop.is_set():
            return
        delay = epoch + (received - recording.first_ns) / 1e9 - time.monotonic()
        if delay > 0 and stop.wait(delay):
            return
        send_packet(video, with_camera_id(body, camera_id))
        sent += 1
    stop.wait(1.0)
    print(f"{recording.path.name}: camera id {camera_id}、{sent} パケット完了", flush=True)


def replay(recordings: list[RecordingSlice], poses: dict, host: str) -> None:
    stop = threading.Event()
    with contextlib.ExitStack() as stack:
        # 登録は直列にして、camera_id の順序を保つ。
        connections = []
        for recording in recordings:
            camera = recording.path.stem.split("_")[0].removeprefix("camera")
            connections.append(open_stream(stack, host, poses[camera]))
        epoch = time.monotonic() + 3.0
        with concurrent.futures.ThreadPoolExecutor(max_workers=len(recordings)) as pool:
            futures = [pool.submit(stream, recording, video, camera_id, epoch, stop)
                       for recording, (video, camera_id) in zip(recordings, connections)]
            try:
                for future in concurrent.futures.as_completed(futures):
                    future.result()
            finally:
                stop.set()


def main() -> int:
    setup_console()
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("dir", type=Path)
    parser.add_argument("--poses", type=Path, required=True)
    parser.add_argument("--from", dest="start", help="録画したときの PC の時計 (HH:MM:SS)")
    parser.add_argument("--seconds", type=float, default=60.0)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--repeat", type=int, default=1, help="同じ区間を接続し直して流す回数 (既定 1)")
    args = parser.parse_args()
    if not math.isfinite(args.seconds) or args.seconds <= 0 or args.repeat < 1:
        parser.error("--seconds は正の有限数、--repeat は 1 以上を指定してください")
    try:
        poses = json.loads(args.poses.read_text(encoding="utf-8"))
        recordings = [path for path in sorted((args.dir / "video").glob("camera*.vsp"))
                      if path.stem.split("_")[0].removeprefix("camera") in poses]
        if not recordings:
            raise ValueError("poses.json の camera_id に合う .vsp がありません")
        with recordings[0].open("rb") as source:
            first_ns = struct.unpack("<q", source.read(8))[0]
        if args.start:
            day = datetime.fromtimestamp(first_ns / 1e9).date()
            start_ns = int(datetime.combine(day, datetime.strptime(args.start, "%H:%M:%S").time()).timestamp() * 1e9)
        else:
            start_ns = first_ns
        end_ns = start_ns + int(args.seconds * 1e9)
        indexed = [index_vsp(path, start_ns, end_ns) for path in recordings]
        for iteration in range(args.repeat):
            print(f"再生 {iteration + 1}/{args.repeat}", flush=True)
            replay(indexed, poses, args.host)
        return 0
    except (OSError, ValueError, KeyError, TypeError, struct.error) as error:
        print(f"動画の再送に失敗しました: {error}", file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        print("動画の再送を中断しました", file=sys.stderr)
        return 130


if __name__ == "__main__":
    raise SystemExit(main())
