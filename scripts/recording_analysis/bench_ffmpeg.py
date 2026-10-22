#!/usr/bin/env python3
"""ball_tracker と同じ設定で ffmpeg に録画をデコードさせ、GPU の方式と同時に動かす本数ごとに CPU の時間を測る。

    python bench_ffmpeg.py <camera0.h264> <camera1.h264> [--frames 2400] [--hwaccel none d3d11va d3d12va dxva2] [--csv out.csv]

ball_tracker は、カメラ 1 台ごとに ffmpeg を 1 本起動する (`ball_tracker/src/video/pipeline.rs`)。この ffmpeg の
CPU の時間が、2 本を同時に動かすと 1 本のときの約 3 倍になった (2026-10-19、Ryzen 7 6800U、d3d11va)。どの方式で
起きるのか、デコードと縮小のどちらで起きるのか、120fps で流したときにも起きるのかを切り分けるのに使う。

次のすべての組み合わせを、`--repeat` の回数だけ繰り返す。同時に動かす n 本目には、n 番目の録画を使う (足りなければ
初めから使い回す)。

`gpuscale` は GPU 上で縮小する出力方式で、`d3d11va` と `d3d12va` だけに対応する。それ以外の方式では実行しない。
2026-10-22 時点の ffmpeg 9.0.2 では `scale_d3d11` は Snapdragon X と Ryzen 7 6800U の両方で
「Could not create the texture (80070057)」となり、`d3d12va` と `scale_d3d12` は Snapdragon X で動作した。

- GPU の方式 (`--hwaccel`): ffmpeg の `-hwaccel`。`none` は CPU でデコードする
- 出力 (`--outputs`): `full` は縮小して RGB にする。`decode` はデコードだけ。`yuv` は `full` と同じ縮小で
  YUV 420 にする。`nv12` は縮小せず NV12 で出す。`gpuscale` は GPU で縮小してから CPU に戻し RGB にする
- 速さ (`--pacing`): `max` はできるだけ速く、`realtime` は 120fps の速さで読む (`-re`)。`realtime` でも ffmpeg は
  初めの 0.5 秒ほどを先に読むので、2400 フレームなら `fps` は 123 前後になる
- 本数 (`--streams`): 同時に動かす ffmpeg の数

CPU の時間は ffmpeg の `-benchmark` (ユーザーとカーネルの合計) で測る。GPU の方式の初期化に失敗すると、ffmpeg は
何も言わずに CPU でデコードするので、そのときは `fallback` に印を付ける。

録画の .h264 には時刻がなく、`-re` で速さを合わせられないので、先頭の `--frames` フレームを、120fps の時刻を
付けた .mkv に移してから測る (映像はそのまま。デコードの手間は変わらない)。
"""

from __future__ import annotations

import argparse
import csv
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass, fields
from pathlib import Path

# ball_tracker の `VideoStreamConfig` の既定 (アプリの縮小のあと)
DEFAULT_SIZE = "640x360"
FRAME_RATE = 120

BENCH = re.compile(r"bench: utime=([\d.]+)s stime=([\d.]+)s rtime=([\d.]+)s")
FRAME = re.compile(r"frame=\s*(\d+)")
# gpuscale で使う、方式ごとの GPU 上の画像の形式と縮小のフィルター
GPU_SCALE = {
    "d3d11va": ("d3d11", "scale_d3d11=width={width}:height={height}"),
    "d3d12va": ("d3d12", "scale_d3d12=w={width}:h={height}"),
}
# 方式の初期化に失敗して CPU に切り替えたときのメッセージ (libavcodec の hwaccel_init と ffmpeg の hw_device_setup)
FALLBACK = re.compile(r"Failed setup for format|hwaccel initiali[sz]ation returned error|Device creation failed")


@dataclass
class Result:
    round: int
    hwaccel: str
    output: str
    pacing: str
    streams: int
    stream: int
    video: str
    frames: int
    rtime_s: float
    fps: float
    cpu_ms_per_frame: float
    cores: float
    fallback: bool
    error: str


def remux(ffmpeg: str, video: Path, frames: int, out: Path) -> str:
    """録画の先頭 `frames` フレームを、120fps の時刻を付けて `out` (.mkv) に移す。失敗したら理由を返す。"""
    command = [ffmpeg, "-hide_banner", "-loglevel", "error", "-f", "h264", "-i", str(video), "-frames:v", str(frames)]
    command += ["-c", "copy", "-bsf:v", f"setts=ts=N/({FRAME_RATE}*TB)", "-y", str(out)]
    completed = subprocess.run(
        command, stdin=subprocess.DEVNULL, capture_output=True, text=True, encoding="utf-8", errors="replace"
    )
    if completed.returncode != 0:
        lines = [line for line in completed.stderr.splitlines() if line.strip()]
        return lines[-1] if lines else f"終了コード {completed.returncode}"
    return ""


def ffmpeg_command(
    ffmpeg: str, video: Path, hwaccel: str, output: str, pacing: str, frames: int, size: str
) -> list[str]:
    """ball_tracker の `Pipeline::spawn` と同じ引数に、測るための引数を足したもの。"""
    width, height = size.split("x")
    command = [ffmpeg, "-hide_banner", "-nostats", "-benchmark", "-flags", "low_delay", "-probesize", "32768"]
    command += ["-threads", "1"]
    if hwaccel != "none":
        command += ["-hwaccel", hwaccel]
    if output == "gpuscale":
        # 出力の形式は方式の名前ではなく、GPU 上の画像の形式 (d3d11va なら d3d11)
        command += ["-hwaccel_output_format", GPU_SCALE[hwaccel][0]]
    if pacing == "realtime":
        command += ["-re"]
    command += ["-i", str(video), "-frames:v", str(frames)]
    if output in ("full", "yuv"):
        command += ["-vf", f"scale={width}:{height}:flags=area", "-fps_mode", "passthrough"]
        pixel_format = "rgb24" if output == "full" else "yuv420p"
        command += ["-f", "rawvideo", "-pix_fmt", pixel_format, "-y", os.devnull]
    elif output == "nv12":
        command += ["-fps_mode", "passthrough", "-f", "rawvideo", "-pix_fmt", "nv12", "-y", os.devnull]
    elif output == "gpuscale":
        _, scale = GPU_SCALE[hwaccel]
        command += ["-vf", f"{scale.format(width=width, height=height)},hwdownload,format=nv12"]
        command += ["-fps_mode", "passthrough"]
        command += ["-f", "rawvideo", "-pix_fmt", "rgb24", "-y", os.devnull]
    else:
        command += ["-f", "null", "-"]
    return command


def parse(stderr: str, returncode: int) -> tuple[int, float, float, bool, str]:
    """ffmpeg の stderr から (フレームの数, CPU の秒数, 経過した秒数, CPU に切り替えたか, 誤り) を取り出す。"""
    fallback = FALLBACK.search(stderr) is not None
    frames = FRAME.findall(stderr)
    bench = BENCH.search(stderr)
    if returncode != 0 or bench is None or not frames:
        lines = [line for line in stderr.splitlines() if line.strip()]
        return 0, 0.0, 0.0, fallback, lines[-1] if lines else f"終了コード {returncode}"
    utime, stime, rtime = (float(value) for value in bench.groups())
    return int(frames[-1]), utime + stime, rtime, fallback, ""


def run_streams(
    ffmpeg: str,
    videos: list[tuple[str, Path]],
    hwaccel: str,
    output: str,
    pacing: str,
    streams: int,
    frames: int,
    size: str,
    round_: int,
) -> list[Result]:
    """ffmpeg を `streams` 本、同時に起動して、終わるのを待つ。`videos` は (録画の名前, 移した .mkv)。"""
    processes = []
    for index in range(streams):
        name, video = videos[index % len(videos)]
        command = ffmpeg_command(ffmpeg, video, hwaccel, output, pacing, frames, size)
        # stderr はファイルに書かせる。パイプだと、読んでいない側の ffmpeg がバッファ (Windows では 4KB) を
        # 埋めたところで止まり、同時に動かしたことにならない
        log = tempfile.TemporaryFile()
        process = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=log)
        processes.append((index, name, process, log))
    results = []
    for index, name, process, log in processes:
        process.wait()
        log.seek(0)
        stderr = log.read().decode("utf-8", errors="replace")
        log.close()
        count, cpu_s, rtime_s, fallback, error = parse(stderr, process.returncode)
        if not error and count < frames:
            error = f"{count} フレームしかない"
        results.append(
            Result(
                round=round_,
                hwaccel=hwaccel,
                output=output,
                pacing=pacing,
                streams=streams,
                stream=index,
                video=name,
                frames=count,
                rtime_s=round(rtime_s, 3),
                fps=round(count / rtime_s, 1) if rtime_s > 0 else 0.0,
                cpu_ms_per_frame=round(cpu_s * 1000 / count, 2) if count > 0 else 0.0,
                cores=round(cpu_s / rtime_s, 2) if rtime_s > 0 else 0.0,
                fallback=fallback,
                error=error,
            )
        )
    return results


def print_result(result: Result) -> None:
    mark = " (CPU に切り替わった)" if result.fallback else ""
    if result.error:
        print(f"  {result.stream}: 失敗 {result.error}{mark}", flush=True)
        return
    print(
        f"  {result.stream}: {result.fps:6.1f} fps  CPU {result.cpu_ms_per_frame:6.2f} ms/フレーム  "
        f"{result.cores:4.2f} コア{mark}",
        flush=True,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("videos", nargs="+", type=Path, help="録画の .h264 (ball_tracker の --video-record-dir)")
    parser.add_argument("--ffmpeg", default="ffmpeg", help="ffmpeg の実行ファイル (既定は PATH の ffmpeg)")
    parser.add_argument("--frames", type=int, default=2400, help="1 本でデコードするフレームの数")
    parser.add_argument("--size", default=DEFAULT_SIZE, help="縮小したあとの大きさ (幅x高さ)")
    parser.add_argument("--hwaccel", nargs="+", default=["none", "d3d11va", "dxva2"], help="試す GPU の方式 (d3d12va も指定可能)")
    parser.add_argument("--outputs", nargs="+", default=["full", "decode"], choices=["full", "decode", "yuv", "nv12", "gpuscale"])
    parser.add_argument("--pacing", nargs="+", default=["max", "realtime"], choices=["max", "realtime"])
    parser.add_argument("--streams", nargs="+", type=int, default=[1, 2], help="同時に動かす本数")
    parser.add_argument("--repeat", type=int, default=2, help="すべての組み合わせを繰り返す回数")
    parser.add_argument("--pause", type=float, default=5.0, help="組み合わせのあいだに休む秒数 (熱を逃がす)")
    parser.add_argument("--csv", type=Path, help="結果を書く CSV")
    args = parser.parse_args()

    ffmpeg = shutil.which(args.ffmpeg)
    if ffmpeg is None:
        print(f"ffmpeg が見つからない: {args.ffmpeg}", file=sys.stderr)
        return 1
    missing = [video for video in args.videos if not video.is_file()]
    if missing:
        print(f"録画が見つからない: {', '.join(str(video) for video in missing)}", file=sys.stderr)
        return 1
    if not re.fullmatch(r"\d+x\d+", args.size):
        print(f"--size は 幅x高さ で指定する: {args.size}", file=sys.stderr)
        return 1

    results: list[Result] = []
    with tempfile.TemporaryDirectory(prefix="bench_ffmpeg_") as work:
        videos = []
        for index, video in enumerate(args.videos):
            remuxed = Path(work) / f"{index}.mkv"
            error = remux(ffmpeg, video, args.frames, remuxed)
            if error:
                print(f"録画を .mkv に移せなかった ({video}): {error}", file=sys.stderr)
                return 1
            videos.append((video.name, remuxed))
        for round_ in range(args.repeat):
            for hwaccel in args.hwaccel:
                for output in args.outputs:
                    if output == "gpuscale" and hwaccel not in GPU_SCALE:
                        continue
                    for pacing in args.pacing:
                        for streams in args.streams:
                            print(f"[{round_ + 1}/{args.repeat}] {hwaccel} {output} {pacing} {streams} 本", flush=True)
                            batch = run_streams(
                                ffmpeg, videos, hwaccel, output, pacing, streams, args.frames, args.size, round_
                            )
                            for result in batch:
                                print_result(result)
                            results.extend(batch)
                            time.sleep(args.pause)

    if args.csv is not None:
        with args.csv.open("w", newline="", encoding="utf-8") as file:
            writer = csv.DictWriter(file, fieldnames=[field.name for field in fields(Result)])
            writer.writeheader()
            for result in results:
                writer.writerow(vars(result))
        print(f"結果を書いた: {args.csv}")
    return 0 if all(not result.error for result in results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
