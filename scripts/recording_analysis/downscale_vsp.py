#!/usr/bin/env python3
"""ball_tracker の .vsp 録画を、指定した大きさの H.264 に変換する。"""
import argparse
import os
import shutil
import struct
import subprocess
import sys
import threading
from datetime import datetime
from pathlib import Path


def varint(value: int) -> bytes:
    encoded = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        encoded.append(byte | (0x80 if value else 0))
        if not value:
            return bytes(encoded)


def field_varint(number: int, value: int) -> bytes:
    return varint((number << 3) | 0) + varint(value)


def field_bytes(number: int, value: bytes) -> bytes:
    return varint((number << 3) | 2) + varint(len(value)) + value


def read_varint(data: bytes, position: int) -> tuple[int, int]:
    value = shift = 0
    while position < len(data):
        byte = data[position]
        position += 1
        value |= (byte & 0x7F) << shift
        if byte < 0x80:
            return value, position
        shift += 7
    raise ValueError("Protobuf varint が途中で終わっています")


def fields(data: bytes):
    """Protobuf フィールドを番号、型、生バイト列、値の順で返す。"""
    position = 0
    while position < len(data):
        start = position
        key, position = read_varint(data, position)
        number, wire = key >> 3, key & 7
        if wire == 0:
            value, position = read_varint(data, position)
        elif wire == 2:
            length, position = read_varint(data, position)
            end = position + length
            if end > len(data):
                raise ValueError("Protobuf の長さ付きフィールドが途中で終わっています")
            value = data[position:end]
            position = end
        elif wire == 1:
            if position + 8 > len(data):
                raise ValueError("Protobuf の 64-bit フィールドが途中で終わっています")
            value = data[position:position + 8]
            position += 8
        elif wire == 5:
            if position + 4 > len(data):
                raise ValueError("Protobuf の 32-bit フィールドが途中で終わっています")
            value = data[position:position + 4]
            position += 4
        else:
            raise ValueError(f"Protobuf wire type {wire} に対応していません")
        yield number, wire, data[start:position], value


def replace_fields(data: bytes, replacements: dict[int, bytes]) -> bytes:
    """指定フィールドだけ置換し、未知フィールドを含む残りの protobuf を保つ。"""
    output = bytearray()
    replaced: set[int] = set()
    for number, _, raw, _ in fields(data):
        if number in replacements:
            if number not in replaced:
                output.extend(replacements[number])
                replaced.add(number)
        else:
            output.extend(raw)
    for number, raw in replacements.items():
        if number not in replaced:
            output.extend(raw)
    return bytes(output)


def replace_config(packet: bytes, width: int, height: int, analysis_size: tuple[int, int] | None) -> bytes:
    for number, wire, _, config in fields(packet):
        if number == 1 and wire == 2:
            replacements = {
                4: field_varint(4, width),
                5: field_varint(5, height),
            }
            if analysis_size:
                replacements[7] = field_varint(7, analysis_size[0])
                replacements[8] = field_varint(8, analysis_size[1])
            return replace_fields(packet, {1: field_bytes(1, replace_fields(config, replacements))})
    raise ValueError("VideoStreamConfig パケットの形式が不正です")


def replace_frame(packet: bytes, data: bytes, *, codec_config: bool = False,
                  key_frame: bool = False) -> bytes:
    outer = list(fields(packet))
    frame = next((value for number, wire, _, value in outer if number == 2 and wire == 2), None)
    if frame is None:
        raise ValueError("VideoFrame パケットの形式が不正です")
    replacements = {
        1: field_bytes(1, data),
        2: field_varint(2, int(codec_config)),
        3: field_varint(3, int(key_frame)),
    }
    if codec_config:
        replacements[4] = field_varint(4, 0)
        replacements[5] = field_varint(5, 0)
    new_frame = replace_fields(frame, replacements)
    return replace_fields(packet, {2: field_bytes(2, new_frame)})


def nal_units(data: bytes) -> list[tuple[int, bytes]]:
    starts: list[tuple[int, int]] = []
    position = 0
    while position < len(data):
        found = data.find(b"\x00\x00\x01", position)
        if found < 0:
            break
        begin = found - 1 if found > 0 and data[found - 1] == 0 else found
        starts.append((begin, found + 3))
        position = found + 3
    units = []
    for index, (begin, payload) in enumerate(starts):
        end = starts[index + 1][0] if index + 1 < len(starts) else len(data)
        if payload >= end:
            continue
        units.append((data[payload] & 0x1F, data[begin:end]))
    return units


def starts_picture(unit: bytes) -> bool:
    """スライスの NAL が画像の最初のスライスか (first_mb_in_slice が 0、指数ゴロム符号で先頭のビットが 1)。"""
    header = unit.index(b"\x00\x00\x01") + 3
    return len(unit) > header + 1 and unit[header + 1] & 0x80 != 0


def frame_details(packet: bytes) -> tuple[bytes | None, bool, bool, int, int]:
    frame_data = None
    is_codec = is_key = False
    sensor_ns = timestamp_ms = 0
    for number, wire, _, frame in fields(packet):
        if number == 2 and wire == 2:
            for field, frame_wire, _, value in fields(frame):
                if field == 1 and frame_wire == 2:
                    frame_data = value
                elif field == 2 and frame_wire == 0:
                    is_codec = bool(value)
                elif field == 3 and frame_wire == 0:
                    is_key = bool(value)
                elif field == 4 and frame_wire == 0:
                    sensor_ns = value
                elif field == 5 and frame_wire == 0:
                    timestamp_ms = value
            break
    return frame_data, is_codec, is_key, sensor_ns, timestamp_ms


def read_records(path: Path, start_ns: int | None, end_ns: int | None):
    head: list[tuple[int, bytes]] = []
    selected: list[tuple[int, bytes]] = []
    first_ns = None
    with path.open("rb") as source:
        while True:
            header = source.read(12)
            if not header:
                break
            if len(header) != 12:
                raise ValueError(".vsp の末尾に不完全なヘッダーがあります")
            received_ns, length = struct.unpack("<qI", header)
            packet = source.read(length)
            if len(packet) != length:
                raise ValueError(".vsp の末尾に不完全なパケットがあります")
            if first_ns is None:
                first_ns = received_ns
            top = list(fields(packet))
            is_config = any(n == 1 and w == 2 for n, w, _, _ in top)
            frame = frame_details(packet) if any(n == 2 and w == 2 for n, w, _, _ in top) else None
            is_codec = frame is not None and frame[1]
            if start_ns is not None and received_ns < start_ns:
                if is_config or is_codec:
                    head.append((received_ns, packet))
                continue
            if end_ns is not None and received_ns > end_ns:
                break
            selected.append((received_ns, packet))
    if first_ns is None:
        raise ValueError("入力 .vsp にパケットがありません")
    return first_ns, head, selected


def split_size(value: str) -> tuple[int, int]:
    try:
        width_text, height_text = value.lower().split("x", 1)
        width, height = int(width_text), int(height_text)
    except ValueError as error:
        raise argparse.ArgumentTypeError("大きさは WxH で指定してください") from error
    if width <= 0 or height <= 0 or width % 2 or height % 2:
        raise argparse.ArgumentTypeError("幅と高さは正の偶数にしてください")
    return width, height


def parse_time(value: str, first_ns: int) -> int:
    try:
        time_of_day = datetime.strptime(value, "%H:%M:%S").time()
    except ValueError as error:
        raise ValueError("--from は HH:MM:SS で指定してください") from error
    date = datetime.fromtimestamp(first_ns / 1e9).date()
    return int(datetime.combine(date, time_of_day).timestamp() * 1e9)


def run_ffmpeg(ffmpeg: str, frames: list[bytes], width: int, height: int,
               bits_per_pixel: float) -> bytes:
    bitrate = int(width * height * 120 * bits_per_pixel)
    command = [ffmpeg, "-v", "error", "-f", "h264", "-framerate", "120", "-i", "pipe:0", "-vf",
               f"scale={width}:{height}:flags=area", "-c:v", "libx264", "-preset", "veryfast",
               "-tune", "zerolatency", "-bf", "0", "-g", "120", "-keyint_min", "120",
               "-sc_threshold", "0", "-x264-params", "repeat-headers=1:sliced-threads=0", "-b:v", str(bitrate),
               "-maxrate", str(bitrate), "-bufsize", str(bitrate), "-pix_fmt", "yuvj420p",
               # 生の H.264 には時刻がないので 120fps と決める。エンコーダーはこの間隔でビットレートを配る
               # (決めないと 1 フレームに数十バイトしか使わない)。枚数は下で入力と照らし合わせる
               "-r", "120", "-f", "h264", "pipe:1"]
    process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    errors: list[BaseException] = []

    def write_input() -> None:
        try:
            assert process.stdin is not None
            for index, frame in enumerate(frames, start=1):
                process.stdin.write(frame)
                if index % 600 == 0:
                    print(f"ffmpeg に {index}/{len(frames)} フレームを入力しました", file=sys.stderr)
            process.stdin.close()
        except (BrokenPipeError, OSError) as error:
            errors.append(error)

    writer = threading.Thread(target=write_input, name="ffmpeg-input")
    writer.start()
    assert process.stdout is not None and process.stderr is not None
    output = bytearray()
    stderr = bytearray()

    def read_stderr() -> None:
        stderr.extend(process.stderr.read())

    stderr_reader = threading.Thread(target=read_stderr, name="ffmpeg-stderr")
    stderr_reader.start()
    output.extend(process.stdout.read())
    writer.join()
    stderr_reader.join()
    return_code = process.wait()
    if return_code:
        detail = stderr.decode("utf-8", errors="replace").strip()
        raise RuntimeError(f"ffmpeg で再エンコードできませんでした: {detail or return_code}")
    if errors:
        raise RuntimeError(f"ffmpeg への入力に失敗しました: {errors[0]}")
    return bytes(output)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path)
    parser.add_argument("output_dir", type=Path)
    parser.add_argument("--size", required=True, type=split_size)
    parser.add_argument("--from", dest="start")
    parser.add_argument("--seconds", type=float, default=60.0)
    parser.add_argument("--bits-per-pixel", type=float, default=0.25)
    parser.add_argument("--analysis-size", type=split_size)
    args = parser.parse_args()
    try:
        if args.seconds <= 0 or args.bits_per_pixel <= 0:
            raise ValueError("--seconds と --bits-per-pixel は正の値にしてください")
        ffmpeg = shutil.which(os.environ.get("FFMPEG", "ffmpeg"))
        if ffmpeg is None:
            raise FileNotFoundError("PATH 上に ffmpeg がありません")
        with args.input.open("rb") as source:
            header = source.read(8)
        if len(header) != 8:
            raise ValueError("入力 .vsp にパケットがありません")
        first_ns = struct.unpack("<q", header)[0]
        start_ns = parse_time(args.start, first_ns) if args.start else None
        end_ns = start_ns + int(args.seconds * 1e9) if start_ns is not None else None
        if start_ns is None:
            start_ns = first_ns
            end_ns = first_ns + int(args.seconds * 1e9)
        _, head, selected = read_records(args.input, start_ns, end_ns)
        if not any(any(n == 1 and w == 2 for n, w, _, _ in fields(packet))
                   for _, packet in head + selected):
            raise ValueError("指定範囲より前にも範囲内にも VideoStreamConfig がありません")
        source_frames: list[tuple[int, bytes, bytes, int, int, bool]] = []
        prior_config_nals: list[bytes] = []
        for received, packet in head + selected:
            if any(n == 1 and w == 2 for n, w, _, _ in fields(packet)):
                continue
            frame_data, codec, key, sensor_ns, timestamp_ms = frame_details(packet)
            if frame_data is None:
                raise ValueError("VideoFrame にデータがありません")
            units = nal_units(frame_data)
            if codec:
                prior_config_nals.extend(unit for kind, unit in units if kind in (7, 8))
                continue
            source_frames.append((received, packet, frame_data, sensor_ns, timestamp_ms,
                                  key or any(kind == 5 for kind, _ in units)))
        if not source_frames:
            raise ValueError("指定範囲に動画フレームがありません")
        first_key = next((i for i, frame in enumerate(source_frames) if frame[5]), None)
        if first_key is None:
            raise ValueError("指定範囲の後にキーフレームがありません")
        source_frames = source_frames[first_key:]
        prefix = b"".join(prior_config_nals)
        if not prefix:
            # 録画範囲のフレーム自体に SPS/PPS が含まれる場合は、その設定を使う。
            prefix = b"".join(unit for kind, unit in nal_units(source_frames[0][2]) if kind in (7, 8))
        ffmpeg_input = [prefix + source_frames[0][2], *(frame[2] for frame in source_frames[1:])]
        print(f"ffmpeg に {len(ffmpeg_input)} フレームを入力します", file=sys.stderr)
        encoded = run_ffmpeg(ffmpeg, ffmpeg_input, *args.size, args.bits_per_pixel)
        output_units = nal_units(encoded)
        frames: list[tuple[bytes, bool]] = []
        pending: list[bytes] = []
        initial_config: list[bytes] = []
        for kind, unit in output_units:
            if kind in (7, 8):
                pending.append(unit)
                if len(initial_config) < 2:
                    initial_config.append(unit)
            elif kind in (1, 5):
                if starts_picture(unit) or not frames:
                    frames.append((b"".join(pending) + unit, kind == 5))
                else:
                    # 同じ画像の 2 つ目以降のスライス
                    data, key = frames[-1]
                    frames[-1] = (data + b"".join(pending) + unit, key or kind == 5)
                pending.clear()
            else:
                pending.append(unit)
        if len(frames) != len(source_frames):
            raise ValueError(f"フレーム数が一致しません (入力 {len(source_frames)}、出力 {len(frames)})")
        if len(initial_config) != 2:
            raise ValueError("ffmpeg の出力に SPS/PPS がありません")

        destination = args.output_dir / "video"
        destination.mkdir(parents=True, exist_ok=True)
        vsp_path = destination / args.input.name
        h264_path = vsp_path.with_suffix(".h264")
        with vsp_path.open("wb") as vsp, h264_path.open("wb") as h264:
            head_configs = [(received, packet) for received, packet in head
                            if any(n == 1 and w == 2 for n, w, _, _ in fields(packet))]
            for _, packet in head_configs:
                rewritten = replace_config(packet, *args.size, args.analysis_size)
                vsp.write(struct.pack("<qI", source_frames[0][0], len(rewritten)))
                vsp.write(rewritten)
            first = source_frames[0]
            codec_packet = replace_frame(first[1], b"".join(initial_config), codec_config=True)
            vsp.write(struct.pack("<qI", first[0], len(codec_packet)))
            vsp.write(codec_packet)
            skipped_frames = first_key
            output_frame = 0
            for received, packet in selected:
                top = list(fields(packet))
                if any(n == 1 and w == 2 for n, w, _, _ in top):
                    rewritten = replace_config(packet, *args.size, args.analysis_size)
                elif any(n == 2 and w == 2 for n, w, _, _ in top):
                    _, codec, _, _, _ = frame_details(packet)
                    if codec:
                        continue
                    if skipped_frames:
                        skipped_frames -= 1
                        continue
                    data, key = frames[output_frame]
                    packet = replace_frame(packet, data, key_frame=key)
                    output_frame += 1
                    h264.write(data)
                    rewritten = packet
                else:
                    rewritten = packet
                vsp.write(struct.pack("<qI", received, len(rewritten)))
                vsp.write(rewritten)
        print(f"完了: {len(frames)} フレーム、{vsp_path.stat().st_size:,} bytes (.vsp)、"
              f"{h264_path.stat().st_size:,} bytes (.h264)", file=sys.stderr)
        return 0
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"エラー: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
