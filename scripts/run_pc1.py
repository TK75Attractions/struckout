#!/usr/bin/env python3
"""PC① をひとまとめに起動する: ball_tracker を起動し、スマホに設定を入れて開き、確かめるところまで。

    python scripts/run_pc1.py
    python scripts/run_pc1.py --output csv --name test
    python scripts/run_pc1.py --record                          # 受け取った動画も残す (実験用)
    python scripts/run_pc1.py -- --time-trim-ms 3               # ball_tracker にオプションを足す

次の順に進め、それぞれ ball_tracker のログで済んだことを確かめてから次へ進む。

1. 電源、ffmpeg、adb、スマホ (scripts/cameras.json の端末)、ディスクの空きを確かめる
2. スマホのアプリをいったん止め、adb reverse (6060、5050、5052) を張り、設定 (cameras.json) を入れる。
   このとき端末の識別子 (device_id) を cameras.json に記録する
3. ball_tracker を scripts/pc1.json の設定と --cameras scripts/cameras.json で起動する。
   ball_tracker は device_id で端末を見分け、cameras.json の順の camera_id (0, 1, ...) を振り、位置と焦点距離を端末に送る
4. (output が network のとき) PC② に入れるつなぎ先を表示する。projector は待たない (いつつないでもよい)
5. スマホを開き、動画が届くのを確かめる
6. 届いている fps と遅れを表にし、Save frame を撮って取り込む
7. 投球中は、着弾と警告、projector とスマホのつながり具合だけを表示する。Ctrl+C で止めると、記録をまとめ、
   スマホのアプリを止める

スマホのアプリと projector は、ball_tracker が止まっても自分でつなぎ直すので、ball_tracker だけを起動し直してもよい
(このスクリプトをもう一度実行する)。

記録は recordings/<日付>_<時分>[_<name>] に残す (ball_tracker のログ、-j の記録、Save frame。--record のときは
受け取った動画も)。動画は 2 台で 1 時間に約 45 GB になるので、稼働中は残さない。ディスクの空きは起動時と 1 分おきに確かめる。
"""

from __future__ import annotations

import argparse
import collections
import os
import re
import shutil
import socket
import subprocess
import sys
import time
from datetime import datetime
from pathlib import Path

from camera_remote import CameraRemote
from struckout_common import (ROOT, SCRIPTS, Failure, exe, fail, find_ffmpeg, free_gb, load_json, local_ipv4,
                              note, ok, on_battery, report_failure, setup_console, step, tracker_data_dir, warn)

# 動画は 2 台で 1 時間に約 45 GB (2026-10-16 の実測: 8.5 分で 6.4 GB)
VIDEO_GB_PER_HOUR = 45

STATS = re.compile(r'video analysis camera_id=(\d+) fps="([\d.]+)".*delay_ms_p50=(\d+) delay_ms_p95=(\d+) '
                   r'max_delay_ms=(\d+)')
LATENCY = re.compile(r"camera clock camera_id=(\d+).*min_latency_ms=([\d.]+)")


def short(line: str) -> str:
    """ログの行から、時刻、レベル、モジュール名を除く。"""
    return re.sub(r"^\S+\s+\w+\s+ball_tracker::", "", line)


class TrackerLog:
    """ball_tracker のログを読み進める。読んだところまでを覚えておき、新しい行だけを返す。"""

    def __init__(self, path: Path, process: subprocess.Popen):
        self.path = path
        self.process = process
        self.offset = 0
        self.rest = b""
        # 待っていた行より後ろの行も捨てない
        self.pending: collections.deque[str] = collections.deque()

    def _read_file(self) -> None:
        if not self.path.exists():
            return
        with open(self.path, "rb") as f:
            f.seek(self.offset)
            data = f.read()
        self.offset += len(data)
        parts = (self.rest + data).split(b"\n")
        self.rest = parts.pop()
        for part in parts:
            self.pending.append(part.decode("utf-8", errors="replace").rstrip("\r"))

    def new_lines(self) -> list[str]:
        self._read_file()
        lines = list(self.pending)
        self.pending.clear()
        return lines

    def wait(self, pattern: str, timeout: float, waiting_text: str | None = None) -> str | None:
        """見たい行が出るまで待つ。途中の警告は表示する。timeout が 0 なら待ち続ける。"""
        regex = re.compile(pattern)
        deadline = time.monotonic() + timeout if timeout > 0 else float("inf")
        last_note = time.monotonic()
        while time.monotonic() < deadline:
            self._read_file()
            while self.pending:
                line = self.pending.popleft()
                if " WARN " in line or " ERROR " in line:
                    warn(short(line))
                if regex.search(line):
                    return line
            if self.process.poll() is not None:
                fail(f"ball_tracker が終了しました (ログ: {self.path})")
            if waiting_text and time.monotonic() - last_note >= 15:
                note(waiting_text)
                last_note = time.monotonic()
            time.sleep(0.3)
        return None


def precheck(record_video: bool):
    step("前提を確かめる")
    tracker = ROOT / "target" / "release" / exe("ball_tracker")
    if not tracker.exists():
        fail(f"ball_tracker がビルドされていません: {tracker} (docs/setup_pc1.md の 3 章)")
    ok(f"ball_tracker: {tracker}")
    if on_battery():
        warn("バッテリーで動いています。動画の解析が 120fps に追いつかないことがあるので、電源につないでください")
    ffmpeg = find_ffmpeg() or fail("ffmpeg が見つかりません (Windows: winget install --id Gyan.FFmpeg -e、Linux: apt install ffmpeg)")
    ok(f"ffmpeg: {ffmpeg}")
    remote = CameraRemote(quiet=True)
    for camera in remote.config["cameras"]:
        if camera["serial"] in remote.connected:
            ok(f"スマホ {camera['name']}: {camera['serial']}")
        else:
            warn(f"スマホ {camera['name']} ({camera['serial']}) がつながっていません")
    if not remote.cameras:
        fail("cameras.json の端末が 1 台もつながっていません")

    free = free_gb(ROOT)
    if record_video:
        hours = round(free / VIDEO_GB_PER_HOUR, 1)
        if free < 20:
            fail(f"ディスクの空きが {free} GB しかありません。--record を外すか、recordings の古い動画を移してください")
        elif hours < 3:
            warn(f"ディスクの空き {free} GB: 動画を残すと約 {hours} 時間で埋まります")
        else:
            ok(f"ディスクの空き {free} GB (動画を残して約 {hours} 時間ぶん)")
    else:
        if free < 2:
            fail(f"ディスクの空きが {free} GB しかありません")
        ok(f"ディスクの空き {free} GB (動画は残さない。残すときは --record)")
    return tracker, ffmpeg, remote


def prepare_phones(remote: CameraRemote) -> None:
    step("スマホのアプリを止め、adb reverse を張り、設定を入れる")
    for camera in remote.cameras:
        remote.stop_one(camera)
        for port in (6060, 5050, 5052):
            subprocess.run([remote.adb, "-s", camera["serial"], "reverse", f"tcp:{port}", f"tcp:{port}"],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        ok(f"{camera['name']}: 6060、5050、5052")
    # 端末の識別子 (device_id) も cameras.json に記録される。ball_tracker はこれで端末を見分ける
    for line in remote.set_config():
        print(f"  {line}")
    for camera in remote.cameras:
        if not camera.get("device_id"):
            warn(f"{camera['name']} の device_id が分かりません。ball_tracker はこの端末をつないだ順の番号で扱います")


def tracker_arguments(settings, output: str, ffmpeg: str, run_dir: Path, record_video: bool, extra: list[str]):
    args = ["--input", "network", "--output", output, "-j",
            "--cameras", SCRIPTS / "cameras.json",
            "--ffmpeg", ffmpeg,
            "--video-hwaccel", settings["video_hwaccel"],
            "--report-delay-ms", settings["report_delay_ms"],
            "--time-trim-ms", settings["time_trim_ms"],
            "--rolling-shutter-skew-ms", settings["rolling_shutter_skew_ms"],
            "--detection-center", settings["detection_center"],
            "--edge-detections", settings["edge_detections"],
            "--video-snapshot-dir", run_dir / "snapshots"]
    if settings.get("auto_time_trim"):
        args.append("--auto-time-trim")
    if record_video:
        args += ["--video-record-dir", run_dir / "video"]
    if output == "csv":
        args += ["-c", run_dir / "collisions_raw.csv"]
    return [str(a) for a in args] + extra


def open_phones(log: TrackerLog, remote: CameraRemote) -> dict[int, str]:
    step("スマホを開く")
    serials = [c["serial"] for c in remote.config["cameras"]]
    ids: dict[int, str] = {}
    for camera in remote.cameras:
        # device_id が分かっていれば、camera_id は cameras.json の順番になる
        index = serials.index(camera["serial"])
        id_pattern = rf"{index}\b" if camera.get("device_id") else r"\d+"
        camera_id = None
        for _ in range(2):
            remote.open_one(camera)
            line = log.wait(rf"received video stream config camera_id={id_pattern}", 40, f"{camera['name']} を待っています")
            match = re.search(r"camera_id=(\d+)", line or "")
            if match:
                camera_id = int(match.group(1))
                break
            warn(f"{camera['name']} から動画が届きません。アプリを開き直します")
            remote.stop_one(camera)
            time.sleep(2)
        if camera_id is None:
            fail(f"{camera['name']} ({camera['serial']}) がつながりません")
        ids[camera_id] = camera["name"]
        ok(f"{camera['name']} = camera_id {camera_id}")
    return ids


def check_streams(log: TrackerLog, ids: dict[int, str], stats: dict) -> None:
    step("届いている動画を確かめる (約 12 秒)")
    latency: dict[int, float] = {}
    until = time.monotonic() + 25
    while time.monotonic() < until and len(stats) < len(ids):
        for line in log.new_lines():
            match = STATS.search(line)
            if match:
                stats[int(match.group(1))] = match.groups()
            match = LATENCY.search(line)
            if match:
                latency[int(match.group(1))] = float(match.group(2))
            if " WARN " in line:
                warn(short(line))
        time.sleep(0.5)
    for camera_id in sorted(ids):
        s = stats.get(camera_id)
        if not s:
            warn(f"camera_id {camera_id} ({ids[camera_id]}): まだ統計が出ていません")
            continue
        fps = float(s[1])
        text = (f"camera_id {camera_id} ({ids[camera_id]}): {fps} fps、PC での遅れ 中央値 {s[2]} ms / 95% {s[3]} ms / "
                f"最大 {s[4]} ms")
        if camera_id in latency:
            text += f"、スマホから PC まで最短 {round(latency[camera_id])} ms"
        (warn if fps < 110 or int(s[3]) > 100 else ok)(text)


def watch(log: TrackerLog, process: subprocess.Popen, stats: dict, record_video: bool) -> None:
    step("準備完了。投げてください (止めるときは Ctrl+C)")
    collisions = 0
    last_status = time.monotonic()
    while process.poll() is None:
        now = datetime.now().strftime("%H:%M:%S")
        for line in log.new_lines():
            if "detected collision" in line:
                collisions += 1
                print(f"  {now}  着弾 {collisions} 件目", flush=True)
            elif "accepted TCP connection from projector" in line:
                ok(f"{now}  projector がつながった")
            elif "no projector is connected" in line:
                warn(f"{now}  projector がつながっていないので、着弾を的に送れませんでした")
            elif re.search(r"camera connected camera_id=(\d+)", line):
                ok(f"{now}  スマホ camera_id {re.search(r'camera_id=(\d+)', line).group(1)} がつながった")
            elif "automatic time trim" in line:
                match = re.search(r'trim_ms="([-\d.]+)"', line)
                if match:
                    note(f"右のカメラの時刻の自動補正: {match.group(1)} ms")
            elif re.search(r" WARN |disconnected|lost", line):
                warn(short(line))
            else:
                match = STATS.search(line)
                if match:
                    stats[int(match.group(1))] = match.groups()
        if time.monotonic() - last_status >= 60:
            last_status = time.monotonic()
            parts = [f"camera_id {i} {stats[i][1]} fps (遅れ 95% {stats[i][3]} ms)" for i in sorted(stats)]
            note(f"{now}  着弾 {collisions} 件、{'、'.join(parts)}")
            free = free_gb(ROOT)
            if record_video and free < 20:
                warn(f"ディスクの空きが {free} GB です (動画で 1 時間に約 {VIDEO_GB_PER_HOUR} GB 減る)。"
                     "Ctrl+C で止め、--record なしで起動し直してください")
            elif free < 2:
                warn(f"ディスクの空きが {free} GB です")
        time.sleep(0.3)
    warn(f"ball_tracker が終了しました (ログ: {log.path})")


def main() -> int:
    setup_console()
    parser = argparse.ArgumentParser(description="PC① をひとまとめに起動する")
    parser.add_argument("--output", choices=["network", "csv"],
                        help="pc1.json の output を上書きする。csv は projector なしで試すとき (着弾は CSV に書く)")
    parser.add_argument("--name", help="記録フォルダーの名前に付ける言葉")
    parser.add_argument("--no-save-frame", action="store_true", help="Save frame を撮らない")
    parser.add_argument("--record", action="store_true",
                        help="受け取った動画を残す (pc1.json の record_video より優先)。2 台で 1 時間に約 45 GB")
    parser.add_argument("extra", nargs="*", help="-- のあとに書いたものは、そのまま ball_tracker に渡す")
    args = parser.parse_args()

    settings = load_json(SCRIPTS / "pc1.json")
    output = args.output or settings["output"]
    record_video = args.record or bool(settings.get("record_video"))

    process = None
    remote = None
    run_dir = None
    started = time.time()
    try:
        tracker, ffmpeg, remote = precheck(record_video)
        prepare_phones(remote)

        step("ball_tracker を起動する")
        stamp = datetime.now().strftime("%Y-%m-%d_%H%M")
        run_dir = ROOT / "recordings" / (stamp + (f"_{args.name}" if args.name else ""))
        run_dir.mkdir(parents=True, exist_ok=True)
        log_path = run_dir / "tracker.log"
        env = dict(os.environ, NO_COLOR="1")
        command = [str(tracker)] + tracker_arguments(settings, output, ffmpeg, run_dir, record_video, args.extra)
        # Ctrl+C はこのスクリプトだけが受け取り、ball_tracker は後片付けで止める (記録を写す前に落ちないように)
        group = ({"creationflags": subprocess.CREATE_NEW_PROCESS_GROUP} if os.name == "nt"
                 else {"start_new_session": True})
        with open(log_path, "wb") as out, open(run_dir / "tracker_stderr.log", "wb") as err:
            process = subprocess.Popen(command, stdout=out, stderr=err, env=env, stdin=subprocess.DEVNULL, **group)
        ok(f"記録フォルダー: {run_dir}")
        log = TrackerLog(log_path, process)

        hw = log.wait(r"video::hwaccel", 30)
        if hw:
            ok("デコード: " + re.sub(r" available=.*", "", re.sub(r"^.*video::hwaccel: ", "", hw)))

        if output == "network":
            step("PC② の projector のつなぎ先")
            if not log.wait(r"listening for projector", 30):
                fail("ball_tracker が projector の待ち受けを始めません")
            addresses = "、".join(local_ipv4()) or "(分かりません)"
            print(f"  PC② で scripts/run_pc2.py を実行してください (pc2.json の tracker_host: このPCの名前 "
                  f"{socket.gethostname()}、または {addresses})")
            print("  projector はいつつないでもよい。つながったら下に表示する")
        if not log.wait(r"starting TCP listener", 30):
            fail("ball_tracker がスマホの受け付けを始めません")
        ok("スマホの受け付けを始めた")

        ids = open_phones(log, remote)
        stats: dict = {}
        check_streams(log, ids, stats)
        for line in remote.status():
            (ok if "tcp=connected camera_screen=true" in line else warn)(line)
        if settings.get("save_frame") and not args.no_save_frame:
            step("Save frame を撮って取り込む (向きの較正用。筐体の中に人がいない状態で)")
            for line in remote.save_frame(run_dir / "calibration"):
                print(f"  {line}")

        watch(log, process, stats, record_video)
        return 0
    except Failure as e:
        return report_failure(e)
    except KeyboardInterrupt:
        print()
        note("止めます")
        return 0
    finally:
        if process is not None and process.poll() is None:
            process.terminate()
            try:
                process.wait(5)
            except subprocess.TimeoutExpired:
                process.kill()
        if run_dir is not None:
            # この回の -j の記録を写す
            json_dir = tracker_data_dir() / "json"
            if json_dir.exists():
                for path in json_dir.iterdir():
                    if re.match(r"^(events|frames)_", path.name) and path.stat().st_mtime >= started:
                        shutil.copy2(path, run_dir / path.name)
        if remote is not None:
            # 使い終わったスマホは、熱を持たないようにアプリを止める
            for camera in remote.cameras:
                remote.stop_one(camera)
        if run_dir is not None:
            print()
            print(f"記録: {run_dir}")


if __name__ == "__main__":
    sys.exit(main())
