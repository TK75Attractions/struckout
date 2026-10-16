#!/usr/bin/env python3
"""ball_tracker が保存した動画 (`--video-record-dir` の .vsp) を、スマホが送ったときと同じ間隔で ball_tracker に流し直す。

    python replay_stream_to_tracker.py <記録フォルダー> --poses <poses.json> [--from 12:29:30] [--seconds 60]

スマホの代わりに、カメラの登録 (6060 番) と動画の送信 (5052 番) を行う。送るのはスマホが実際に送ったパケット
そのもの (設定の camera_id だけ、ball_tracker が新しく振った番号に書き換える)。間隔は、録画したときに PC が
受け取った時刻の間隔に合わせる。PC での解析の遅れ (デコードの速さなど) を、実機なしで確かめるのに使う。
時刻合わせには答えないので、ball_tracker は受け取った時刻から時計を見積もる (遅れの確かめには影響しない)。

先に ball_tracker を起動しておく (`--input network --output csv`)。poses.json は replay_video と同じ形。
"""
import argparse
import json
import socket
import struct
import threading
import time
from datetime import datetime
from pathlib import Path

from replay_to_tracker import encode_camera_pose, field_varint, receive_camera_id, send_packet


def read_varint(data, pos):
    value = shift = 0
    while True:
        byte = data[pos]
        pos += 1
        value |= (byte & 0x7F) << shift
        shift += 7
        if byte < 0x80:
            return value, pos


def fields(data):
    """Protobuf の最上位のフィールドを (番号, 型, 生のバイト列 (キーを含む), 値) で返す。"""
    pos = 0
    while pos < len(data):
        start = pos
        key, pos = read_varint(data, pos)
        number, wire = key >> 3, key & 7
        if wire == 0:
            value, pos = read_varint(data, pos)
        elif wire == 2:
            length, pos = read_varint(data, pos)
            value = data[pos:pos + length]
            pos += length
        elif wire == 1:
            value = data[pos:pos + 8]
            pos += 8
        elif wire == 5:
            value = data[pos:pos + 4]
            pos += 4
        else:
            raise ValueError(f"unsupported wire type {wire}")
        yield number, wire, data[start:pos], value


def with_camera_id(packet, camera_id):
    """VideoStreamPacket { config { camera_id } } の camera_id を書き換える。設定でなければそのまま返す。"""
    top = list(fields(packet))
    if not top or top[0][0] != 1:
        return packet
    config = b"".join(field_varint(1, camera_id) if number == 1 else raw for number, _, raw, _ in fields(top[0][3]))
    if not any(number == 1 for number, *_ in fields(top[0][3])):
        config = field_varint(1, camera_id) + config
    return b"\x0a" + encode_length(len(config)) + config


def encode_length(n):
    out = bytearray()
    while True:
        byte = n & 0x7F
        n >>= 7
        out.append(byte | (0x80 if n else 0))
        if not n:
            return bytes(out)


def read_vsp(path, start_ns, end_ns):
    """(受け取った時刻 ns, パケット) を返す。範囲の前の設定と SPS/PPS は、範囲の最初に回す。"""
    head, items = [], []
    with open(path, "rb") as fh:
        while True:
            header = fh.read(12)
            if len(header) < 12:
                break
            received, length = struct.unpack("<qI", header)
            body = fh.read(length)
            if len(body) < length:
                break
            top = list(fields(body))
            is_config = top and top[0][0] == 1
            is_codec = top and top[0][0] == 2 and any(n == 2 and v for n, _, _, v in fields(top[0][3]))
            if received < start_ns:
                if is_config or is_codec:
                    head.append(body)
                continue
            if received > end_ns:
                break
            items.append((received, body))
    first = items[0][0] if items else start_ns
    return [(first, b) for b in head] + items


def stream(packets, host, pose, epoch, label):
    control = socket.create_connection((host, 6060))
    camera_id = receive_camera_id(control)
    send_packet(control, encode_camera_pose(camera_id, pose["position_m"], pose["rotation_deg"]))
    video = socket.create_connection((host, 5052))
    video.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
    (length,) = struct.unpack("<I", video.recv(4, socket.MSG_WAITALL))
    answer = video.recv(length, socket.MSG_WAITALL) if length else b""
    if answer != b"\x08\x01":
        print(f"{label}: PC が解析を引き受けませんでした ({answer!r})")
        return
    print(f"{label}: camera id {camera_id}、{len(packets)} パケット")
    base = packets[0][0]
    for received, body in packets:
        delay = epoch + (received - base) / 1e9 - time.time()
        if delay > 0:
            time.sleep(delay)
        send_packet(video, with_camera_id(body, camera_id))
    time.sleep(1.0)
    video.close()
    control.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("dir", type=Path)
    parser.add_argument("--poses", type=Path, required=True)
    parser.add_argument("--from", dest="start", help="流し始める時刻 (録画したときの PC の時計、HH:MM:SS)")
    parser.add_argument("--seconds", type=float, default=60.0)
    parser.add_argument("--host", default="127.0.0.1")
    args = parser.parse_args()
    poses = json.loads(args.poses.read_text(encoding="utf-8"))
    recordings = sorted((args.dir / "video").glob("camera*.vsp"))
    recordings = [p for p in recordings if p.stem.split("_")[0].removeprefix("camera") in poses]
    if not recordings:
        raise SystemExit("poses.json の camera_id に合う .vsp がありません")
    with open(recordings[0], "rb") as fh:
        first_ns = struct.unpack("<q", fh.read(8))[0]
    if args.start:
        day = datetime.fromtimestamp(first_ns / 1e9).date()
        start_ns = int(datetime.combine(day, datetime.strptime(args.start, "%H:%M:%S").time()).timestamp() * 1e9)
    else:
        start_ns = first_ns
    end_ns = start_ns + int(args.seconds * 1e9)
    # 読み込みに時間がかかる (1 本 1GB を超える) ので、全部読んでから同じ時刻に送り始める
    loaded = [(path, read_vsp(path, start_ns, end_ns)) for path in recordings]
    epoch = time.time() + 3.0
    threads = []
    for path, packets in loaded:
        camera = path.stem.split("_")[0].removeprefix("camera")
        thread = threading.Thread(target=stream, args=(packets, args.host, poses[camera], epoch, path.name))
        thread.start()
        threads.append(thread)
        time.sleep(0.5)  # ball_tracker はつないだ順に番号を振るので、順番をそろえる
    for thread in threads:
        thread.join()


if __name__ == "__main__":
    main()
