#!/usr/bin/env python3
"""カメラ用スマホのアプリ (高速撮影版) を、画面に触れずに PC から adb で操作する。

    python scripts/camera_remote.py config       # cameras.json の設定 (出力先、焦点距離、位置と向き) を入れ、端末の識別子を記録する
    python scripts/camera_remote.py open         # アプリをカメラ画面で開く
    python scripts/camera_remote.py status       # 各端末の設定と接続の状態
    python scripts/camera_remote.py save-frame   # 全端末で「Save frame」し、画像を PC に取り込む
    python scripts/camera_remote.py retry        # ball_tracker への接続をやり直す
    python scripts/camera_remote.py stop         # アプリを止める

config は、端末が返す識別子 (device_id) を cameras.json に書き込む。ball_tracker を --cameras cameras.json で起動すると、
これで端末を見分け、つないだ順によらず cameras.json の順の camera_id を振り、位置と焦点距離を端末に送る。
そのため、アプリと ball_tracker のどちらを先に起動してもよく、開く順番も問わない。
端末ごとに 1 回 (アプリを別の PC で署名し直したときも) config を実行しておけばよい。
"""

from __future__ import annotations

import argparse
import re
import sys
import time
from pathlib import Path

from struckout_common import (ROOT, SCRIPTS, Failure, fail, find_adb, load_json, report_failure, run,
                              setup_console, warn)

PREFIX = "com.taichi765.struckoutCameraApp.remote."


class CameraRemote:
    """cameras.json に書いた端末のうち、adb でつながっているものを操作する。"""

    def __init__(self, settings: Path | None = None, quiet: bool = False):
        self.settings = Path(settings or SCRIPTS / "cameras.json")
        self.reload()
        self.adb = find_adb() or fail("adb が見つかりません (Android SDK の platform-tools)")
        lines = run([self.adb, "devices"]).stdout.splitlines()
        self.connected = [line.split("\t")[0] for line in lines if line.endswith("\tdevice")]
        # 書いた順
        self.cameras = [c for c in self.config["cameras"] if c["serial"] in self.connected]
        if not quiet:
            for camera in self.config["cameras"]:
                if camera["serial"] not in self.connected:
                    warn(f"{camera['name']} ({camera['serial']}) がつながっていません")
            for serial in self.connected:
                if not any(c["serial"] == serial for c in self.config["cameras"]):
                    warn(f"{serial} は {self.settings} に書かれていないので操作しません")

    def reload(self) -> None:
        self.config = load_json(self.settings)
        self.package = self.config["package"]
        self.receiver = f"{self.package}/com.taichi765.struckoutCameraApp.remote.RemoteControlReceiver"
        self.activity = f"{self.package}/com.taichi765.struckoutCameraApp.MainActivity"
        if hasattr(self, "connected"):
            self.cameras = [c for c in self.config["cameras"] if c["serial"] in self.connected]

    def adb_shell(self, camera, *args) -> str:
        return run([self.adb, "-s", camera["serial"], "shell", *args]).stdout

    @staticmethod
    def label(camera, text: str) -> str:
        return f"{camera['name']:<6} {camera['serial']}: {text}"

    def broadcast(self, camera, action: str, extras: list[str] | None = None) -> str:
        # -f 32 (FLAG_INCLUDE_STOPPED_PACKAGES): アプリを止めたあとでも届くようにする
        out = self.adb_shell(camera, "am", "broadcast", "-f", "32", "-a", PREFIX + action, "-n", self.receiver,
                             *(extras or []))
        # 結果は「Broadcast completed: result=0, data="..."」の data
        match = re.search(r'data="(.*)"', out)
        return match.group(1) if match else out.strip()

    # --- 命令 ---

    def set_config(self) -> list[str]:
        lines = []
        for camera in self.cameras:
            extras = ["--es", "output", camera["output"],
                      "--es", "focal_ratio", str(camera["focal_ratio"]),
                      "--es", "pose", ",".join(str(v) for v in camera["pose"])]
            if camera.get("area"):
                extras += ["--es", "area", ",".join(str(v) for v in camera["area"])]
            if "stream_size" in camera:
                extras += ["--es", "stream_size", camera["stream_size"]]
            result = self.broadcast(camera, "SET_CONFIG", extras)
            lines.append(self.label(camera, result))
            match = re.search(r"device_id=(\S+)", result)
            if match:
                message = self.save_device_id(camera["serial"], match.group(1))
                if message:
                    lines.append(message)
            else:
                warn(f"{camera['name']} が device_id を返しません (アプリが古い)。ball_tracker はこの端末を見分けられません")
        self.reload()
        return lines

    def save_device_id(self, serial: str, device_id: str) -> str | None:
        """cameras.json の、シリアルが serial の端末に device_id を書く。書式を崩さないよう、行を直接書き換える。"""
        raw = self.settings.read_bytes()
        bom = raw.startswith(b"\xef\xbb\xbf")
        text = raw.decode("utf-8-sig")
        pattern = re.compile(r'^(?P<indent>[ \t]*)"serial"\s*:\s*"' + re.escape(serial) + r'",[ \t]*\r?\n'
                             r'(?:[ \t]*"device_id"\s*:\s*"[^"]*",[ \t]*\r?\n)?', re.MULTILINE)
        match = pattern.search(text)
        if not match:
            warn(f"{self.settings} に {serial} の serial の行が見つからないので、device_id を書けません")
            return None
        newline = "\r\n" if "\r\n" in text else "\n"
        indent = match.group("indent")
        replacement = f'{indent}"serial": "{serial}",{newline}{indent}"device_id": "{device_id}",{newline}'
        if match.group(0) == replacement:
            return None
        text = text[:match.start()] + replacement + text[match.end():]
        self.settings.write_bytes((b"\xef\xbb\xbf" if bom else b"") + text.encode("utf-8"))
        return f"  {serial} の device_id ({device_id}) を {self.settings} に書きました"

    def open(self, wait: float = 3) -> list[str]:
        lines = []
        for i, camera in enumerate(self.cameras):
            self.open_one(camera)
            lines.append(f"{camera['name']} ({camera['serial']}) を開きました")
            if i < len(self.cameras) - 1:
                # 2 台同時に高速撮影を始めると重くなるので、少しあける
                time.sleep(wait)
        return lines

    def open_one(self, camera) -> None:
        self.adb_shell(camera, "am", "start", "-n", self.activity, "--es", "screen", "camera")

    def stop_one(self, camera) -> None:
        self.adb_shell(camera, "am", "force-stop", self.package)

    def status(self) -> list[str]:
        return [self.label(c, self.broadcast(c, "STATUS")) for c in self.cameras]

    def retry(self) -> list[str]:
        return [self.label(c, self.broadcast(c, "RETRY_CONNECTION")) for c in self.cameras]

    def stop(self) -> list[str]:
        lines = []
        for camera in self.cameras:
            self.stop_one(camera)
            lines.append(self.label(camera, "stopped"))
        return lines

    def save_frame(self, dest: Path) -> list[str]:
        lines = [self.label(c, self.broadcast(c, "SAVE_FRAME")) for c in self.cameras]
        # PC が解析しているあいだ、端末は 1 秒に 1 枚しか画像を作らないので、少し待ってから取り込む
        time.sleep(3)
        for camera in self.cameras:
            remote = f"/sdcard/Android/data/{self.package}/files/calibration"
            local = Path(dest) / camera["serial"]
            local.mkdir(parents=True, exist_ok=True)
            run([self.adb, "-s", camera["serial"], "pull", f"{remote}/.", str(local)])
            frames = sorted(local.glob("frame_*.png"))
            lines.append(self.label(camera, str(frames[-1]) if frames else "画像がありません"))
        return lines


def main() -> int:
    setup_console()
    parser = argparse.ArgumentParser(description="カメラ用スマホのアプリを PC から adb で操作する")
    parser.add_argument("command", choices=["config", "open", "status", "save-frame", "retry", "stop"])
    parser.add_argument("--settings", type=Path, help="端末ごとの設定。既定は scripts/cameras.json")
    parser.add_argument("--wait", type=float, default=3, help="open で、1 台開いてから次を開くまでの秒数")
    parser.add_argument("--dest", type=Path, default=ROOT / "recordings" / "calibration",
                        help="save-frame で画像を取り込むフォルダー (端末のシリアルごとのフォルダーを作る)")
    args = parser.parse_args()
    try:
        remote = CameraRemote(args.settings)
        lines = {
            "config": remote.set_config,
            "open": lambda: remote.open(args.wait),
            "status": remote.status,
            "save-frame": lambda: remote.save_frame(args.dest),
            "retry": remote.retry,
            "stop": remote.stop,
        }[args.command]()
        for line in lines:
            print(line)
    except Failure as e:
        return report_failure(e)
    return 0


if __name__ == "__main__":
    sys.exit(main())
