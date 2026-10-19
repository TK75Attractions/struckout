"""scripts/ の起動用スクリプト (run_pc1.py、run_pc2.py、camera_remote.py) が共有する小道具。

標準ライブラリだけを使い、Windows、Linux、macOS で動く。Python 3.9 以上。
"""

from __future__ import annotations

import ctypes
import json
import os
import re
import shutil
import socket
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPTS = ROOT / "scripts"
IS_WINDOWS = os.name == "nt"
IS_MACOS = sys.platform == "darwin"


def platform_key() -> str:
    """設定ファイルで OS ごとに値を分けるときの名前。"""
    if IS_WINDOWS:
        return "windows"
    if IS_MACOS:
        return "macos"
    return "linux"


# --- 表示 ---

def setup_console() -> None:
    """日本語と色が崩れないようにする。"""
    if IS_WINDOWS:
        # Windows 10 以降のコンソールで、色の指定 (ANSI エスケープ) を有効にする
        os.system("")
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")
        except (AttributeError, ValueError):
            pass


def _color(code: str, text: str) -> str:
    if sys.stdout.isatty() and not os.environ.get("NO_COLOR"):
        return f"\x1b[{code}m{text}\x1b[0m"
    return text


def step(text: str) -> None:
    print()
    print(_color("36", f"== {text}"), flush=True)


def ok(text: str) -> None:
    print(_color("32", f"  OK  {text}"), flush=True)


def warn(text: str) -> None:
    print(_color("33", f"  !!  {text}"), flush=True)


def note(text: str) -> None:
    print(f"  ..  {text}", flush=True)


class Failure(Exception):
    """続けられない。メッセージを NG として表示して終わる。"""


def fail(text: str):
    raise Failure(text)


def report_failure(error: Failure) -> int:
    print(_color("31", f"  NG  {error}"), flush=True)
    return 1


# --- ファイル ---

def load_json(path: Path):
    # PowerShell で書いた JSON は先頭に BOM が付くことがある
    return json.loads(Path(path).read_text(encoding="utf-8-sig"))


def per_platform(value):
    """設定の値。OS ごとに {"windows": ..., "linux": ..., "macos": ...} で書いてもよい。"""
    if isinstance(value, dict):
        return value.get(platform_key())
    return value


def read_shared(path: Path) -> str:
    """ほかのプロセスが書いている最中のファイルを読む。読めなければ空。"""
    try:
        with open(path, "rb") as f:
            return f.read().decode("utf-8", errors="replace")
    except OSError:
        return ""


def free_gb(path: Path) -> float:
    return round(shutil.disk_usage(path).free / 1024**3, 1)


# --- ツールを探す ---

def find_adb() -> str | None:
    found = shutil.which("adb")
    if found:
        return found
    name = "adb.exe" if IS_WINDOWS else "adb"
    candidates = [os.environ.get("ANDROID_HOME"), os.environ.get("ANDROID_SDK_ROOT")]
    if IS_WINDOWS:
        candidates.append(os.path.join(os.environ.get("LOCALAPPDATA", ""), "Android", "Sdk"))
    elif IS_MACOS:
        candidates.append(str(Path.home() / "Library" / "Android" / "sdk"))
    else:
        candidates.append(str(Path.home() / "Android" / "Sdk"))
    for sdk in candidates:
        if sdk and (Path(sdk) / "platform-tools" / name).exists():
            return str(Path(sdk) / "platform-tools" / name)
    return None


def find_ffmpeg() -> str | None:
    found = shutil.which("ffmpeg")
    if found:
        return found
    if IS_WINDOWS:
        # winget で入れた直後は PATH に載っていないことがある
        packages = Path(os.environ.get("LOCALAPPDATA", "")) / "Microsoft" / "WinGet" / "Packages"
        if packages.exists():
            for path in packages.rglob("ffmpeg.exe"):
                return str(path)
    return None


def find_mise_tool(tool_dir: str, exe: str) -> str | None:
    """mise で入れたツールの実行ファイル (PATH に載っていないとき)。"""
    found = shutil.which(exe)
    if found:
        return found
    if IS_WINDOWS:
        base = Path(os.environ.get("LOCALAPPDATA", "")) / "mise" / "installs" / tool_dir
        name = exe + ".exe"
    else:
        data = os.environ.get("MISE_DATA_DIR") or os.path.join(
            os.environ.get("XDG_DATA_HOME") or str(Path.home() / ".local" / "share"), "mise")
        base = Path(data) / "installs" / tool_dir
        name = exe
    if base.exists():
        matches = sorted(base.rglob(name))
        if matches:
            return str(matches[-1])
    return None


def tracker_data_dir() -> Path:
    """ball_tracker が -j の記録を書く場所 (Rust の dirs::config_dir() と同じ)。"""
    if IS_WINDOWS:
        base = Path(os.environ["APPDATA"])
    elif IS_MACOS:
        base = Path.home() / "Library" / "Application Support"
    else:
        base = Path(os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config")
    return base / "struckout" / "tracker"


def exe(name: str) -> str:
    return name + ".exe" if IS_WINDOWS else name


# --- コマンド ---

def run(args, cwd=None, env=None, timeout=None) -> subprocess.CompletedProcess:
    """結果を文字列で受け取る。標準エラー出力も stdout にまとめる。"""
    return subprocess.run(
        [str(a) for a in args], cwd=cwd, env=env, timeout=timeout,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL,
        text=True, encoding="utf-8", errors="replace")


def detached_popen(args, **kwargs) -> subprocess.Popen:
    """このスクリプトを Ctrl+C で止めても、端末を閉じても、止まらないように起動する。"""
    if IS_WINDOWS:
        flags = subprocess.CREATE_NEW_PROCESS_GROUP | getattr(subprocess, "DETACHED_PROCESS", 0x8)
        return subprocess.Popen([str(a) for a in args], creationflags=flags, stdin=subprocess.DEVNULL, **kwargs)
    return subprocess.Popen([str(a) for a in args], start_new_session=True, stdin=subprocess.DEVNULL, **kwargs)


def process_running(image: str) -> bool:
    """実行ファイルの名前 (StruckOut.exe など) で、動いているかを調べる。"""
    if IS_WINDOWS:
        out = run(["tasklist", "/FI", f"IMAGENAME eq {image}", "/NH"]).stdout
        return image.lower() in out.lower()
    return run(["pgrep", "-f", re.escape(image)]).returncode == 0


def kill_process(image: str) -> None:
    if IS_WINDOWS:
        run(["taskkill", "/IM", image, "/F"])
    else:
        run(["pkill", "-f", re.escape(image)])


def tcp_ready(host: str, port: int, timeout: float = 1.0) -> bool:
    try:
        with socket.create_connection((host, port), timeout=timeout):
            return True
    except OSError:
        return False


# --- この PC ---

def on_battery() -> bool | None:
    """バッテリーで動いているか。分からなければ None。"""
    if IS_WINDOWS:
        class PowerStatus(ctypes.Structure):
            _fields_ = [("ACLineStatus", ctypes.c_ubyte), ("BatteryFlag", ctypes.c_ubyte),
                        ("BatteryLifePercent", ctypes.c_ubyte), ("SystemStatusFlag", ctypes.c_ubyte),
                        ("BatteryLifeTime", ctypes.c_ulong), ("BatteryFullLifeTime", ctypes.c_ulong)]
        status = PowerStatus()
        if not ctypes.windll.kernel32.GetSystemPowerStatus(ctypes.byref(status)):
            return None
        if status.BatteryFlag == 128:  # バッテリーが無い
            return False
        return {0: True, 1: False}.get(status.ACLineStatus)
    if IS_MACOS:
        out = run(["pmset", "-g", "batt"]).stdout
        return "Battery Power" in out if out else None
    supplies = Path("/sys/class/power_supply")
    if not supplies.exists():
        return None
    mains = [p for p in supplies.iterdir() if (p / "type").exists() and (p / "type").read_text().strip() == "Mains"]
    batteries = [p for p in supplies.iterdir() if (p / "type").exists() and (p / "type").read_text().strip() == "Battery"]
    if not batteries:
        return False
    if not mains:
        return None
    return not any((p / "online").exists() and (p / "online").read_text().strip() == "1" for p in mains)


def local_ipv4() -> list[str]:
    """この PC の IPv4 (ループバックとリンクローカルを除く)。"""
    addresses = set()
    try:
        for info in socket.getaddrinfo(socket.gethostname(), None, socket.AF_INET):
            addresses.add(info[4][0])
    except OSError:
        pass
    try:
        # 送信はしない。外へ出るときに使うアドレスを調べるだけ
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
            s.connect(("192.0.2.1", 9))
            addresses.add(s.getsockname()[0])
    except OSError:
        pass
    return sorted(a for a in addresses if not a.startswith(("127.", "169.254.")))
