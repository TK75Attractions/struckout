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
from functools import lru_cache
from pathlib import Path

from generate_server_ports import ServerPortsError, get_server_port

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


@lru_cache
def server_port(module_name: str, endpoint: str) -> int:
    """ビルド済みモジュールと同じ spec から接続先の既定ポートを読む。"""
    try:
        return get_server_port(module_name, endpoint)
    except (OSError, ServerPortsError) as error:
        raise Failure(f"サーバーポートの定義を読めません: {error}") from error


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


# --- 稼働時の計測 ---


@lru_cache(maxsize=1)
def _windows_runtime_api():
    """型を指定して Win32 API を呼ぶ。64 bit のハンドルを切り詰めないために必要。"""
    from ctypes import wintypes

    class ProcessEntry(ctypes.Structure):
        _fields_ = [("dwSize", wintypes.DWORD), ("cntUsage", wintypes.DWORD),
                    ("th32ProcessID", wintypes.DWORD), ("th32DefaultHeapID", ctypes.c_size_t),
                    ("th32ModuleID", wintypes.DWORD), ("cntThreads", wintypes.DWORD),
                    ("th32ParentProcessID", wintypes.DWORD), ("pcPriClassBase", wintypes.LONG),
                    ("dwFlags", wintypes.DWORD), ("szExeFile", wintypes.WCHAR * 260)]

    class MemoryCounters(ctypes.Structure):
        _fields_ = [("cb", wintypes.DWORD), ("PageFaultCount", wintypes.DWORD)] + [
            (name, ctypes.c_size_t) for name in (
                "PeakWorkingSetSize", "WorkingSetSize", "QuotaPeakPagedPoolUsage", "QuotaPagedPoolUsage",
                "QuotaPeakNonPagedPoolUsage", "QuotaNonPagedPoolUsage", "PagefileUsage", "PeakPagefileUsage",
                "PrivateUsage")]

    class MemoryStatus(ctypes.Structure):
        _fields_ = [("dwLength", wintypes.DWORD), ("dwMemoryLoad", wintypes.DWORD)] + [
            (name, ctypes.c_ulonglong) for name in (
                "ullTotalPhys", "ullAvailPhys", "ullTotalPageFile", "ullAvailPageFile",
                "ullTotalVirtual", "ullAvailVirtual", "ullAvailExtendedVirtual")]

    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    psapi = ctypes.WinDLL("psapi", use_last_error=True)
    signatures = [
        (kernel.CreateToolhelp32Snapshot, [wintypes.DWORD, wintypes.DWORD], wintypes.HANDLE),
        (kernel.Process32FirstW, [wintypes.HANDLE, ctypes.POINTER(ProcessEntry)], wintypes.BOOL),
        (kernel.Process32NextW, [wintypes.HANDLE, ctypes.POINTER(ProcessEntry)], wintypes.BOOL),
        (kernel.OpenProcess, [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD], wintypes.HANDLE),
        (kernel.CloseHandle, [wintypes.HANDLE], wintypes.BOOL),
        (kernel.GetProcessTimes, [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4, wintypes.BOOL),
        (kernel.GetSystemTimes, [ctypes.POINTER(wintypes.FILETIME)] * 3, wintypes.BOOL),
        (kernel.GlobalMemoryStatusEx, [ctypes.POINTER(MemoryStatus)], wintypes.BOOL),
        (psapi.GetProcessMemoryInfo, [wintypes.HANDLE, ctypes.POINTER(MemoryCounters), wintypes.DWORD], wintypes.BOOL),
    ]
    for function, arguments, result in signatures:
        function.argtypes = arguments
        function.restype = result
    return kernel, psapi, ProcessEntry, MemoryCounters, MemoryStatus, wintypes.FILETIME


def _process_name(name: str) -> str:
    return name.lower().removesuffix(".exe")


def _windows_runtime_snapshot(names: set[str]) -> tuple[dict, list[dict]]:
    # https://learn.microsoft.com/windows/win32/psapi/process-memory-usage-information
    kernel, psapi, entry_type, counters_type, status_type, filetime = _windows_runtime_api()

    def ticks(value) -> int:
        return (value.dwHighDateTime << 32) | value.dwLowDateTime

    memory = status_type()
    memory.dwLength = ctypes.sizeof(memory)
    if not kernel.GlobalMemoryStatusEx(ctypes.byref(memory)):
        raise ctypes.WinError(ctypes.get_last_error())
    idle, system_kernel, system_user = filetime(), filetime(), filetime()
    if not kernel.GetSystemTimes(ctypes.byref(idle), ctypes.byref(system_kernel), ctypes.byref(system_user)):
        raise ctypes.WinError(ctypes.get_last_error())
    system = {"total_memory_bytes": memory.ullTotalPhys, "available_memory_bytes": memory.ullAvailPhys,
              "system_cpu_seconds": (ticks(system_kernel) + ticks(system_user)) / 1e7,
              "system_idle_seconds": ticks(idle) / 1e7}
    snapshot = kernel.CreateToolhelp32Snapshot(2, 0)  # TH32CS_SNAPPROCESS
    if snapshot == ctypes.c_void_p(-1).value:
        raise ctypes.WinError(ctypes.get_last_error())
    processes = []
    try:
        entry = entry_type()
        entry.dwSize = ctypes.sizeof(entry)
        present = kernel.Process32FirstW(snapshot, ctypes.byref(entry))
        while present:
            name = entry.szExeFile
            if _process_name(name) in names:
                row = {"name": name, "pid": entry.th32ProcessID, "status": "unavailable",
                       "private_kind": "commit"}
                handle = kernel.OpenProcess(0x1000 | 0x0010, False, entry.th32ProcessID)
                if not handle:
                    # VM_READ を許可しないプロセスでも CPU 時間だけ読めることがある。
                    handle = kernel.OpenProcess(0x1000, False, entry.th32ProcessID)
                if handle:
                    try:
                        created, exited, used_kernel, used_user = filetime(), filetime(), filetime(), filetime()
                        if kernel.GetProcessTimes(handle, ctypes.byref(created), ctypes.byref(exited),
                                                  ctypes.byref(used_kernel), ctypes.byref(used_user)):
                            row.update(started_at=ticks(created),
                                       cpu_seconds=(ticks(used_kernel) + ticks(used_user)) / 1e7,
                                       status="ok")
                        counters = counters_type()
                        counters.cb = ctypes.sizeof(counters)
                        if psapi.GetProcessMemoryInfo(handle, ctypes.byref(counters), counters.cb):
                            row.update(rss_bytes=counters.WorkingSetSize, private_bytes=counters.PrivateUsage)
                        else:
                            row["status"] = "memory_unavailable"
                    finally:
                        kernel.CloseHandle(handle)
                processes.append(row)
            present = kernel.Process32NextW(snapshot, ctypes.byref(entry))
    finally:
        kernel.CloseHandle(snapshot)
    return system, processes


def _linux_runtime_snapshot(names: set[str]) -> tuple[dict, list[dict]]:
    proc = Path("/proc")
    clock_ticks = os.sysconf("SC_CLK_TCK")
    page_size = os.sysconf("SC_PAGE_SIZE")
    memory = {line.split(":", 1)[0]: int(line.split()[1]) * 1024
              for line in (proc / "meminfo").read_text().splitlines() if ":" in line}
    # guest と guest_nice は user/nice に含まれるので二重に足さない。
    cpu = [int(value) for value in (proc / "stat").read_text().splitlines()[0].split()[1:9]]
    system = {"total_memory_bytes": memory["MemTotal"], "available_memory_bytes": memory.get("MemAvailable"),
              "system_cpu_seconds": sum(cpu) / clock_ticks,
              "system_idle_seconds": (cpu[3] + cpu[4]) / clock_ticks}
    processes = []
    for directory in proc.iterdir():
        if not directory.name.isdecimal():
            continue
        try:
            # comm は空白・括弧を含められるので、stat 全体を単純に split しない。
            stat = (directory / "stat").read_text()
            closing = stat.rfind(")")
            fields = stat[closing + 2:].split()
            name = stat[stat.find("(") + 1:closing]
            # comm は 15 bytes に切られる。対象の長い名前は cmdline の実行ファイルから補う。
            command = (directory / "cmdline").read_bytes().split(b"\0", 1)[0]
            executable = Path(os.fsdecode(command)).name if command else name
            if _process_name(executable) not in names and _process_name(name) not in names:
                continue
            row = {"name": executable, "pid": int(directory.name), "status": "ok",
                   "started_at": fields[19], "cpu_seconds": (int(fields[11]) + int(fields[12])) / clock_ticks,
                   "rss_bytes": int(fields[21]) * page_size, "private_kind": "resident"}
            try:
                private_lines = (directory / "smaps_rollup").read_text().splitlines()
                row["private_bytes"] = sum(int(line.split()[1]) * 1024 for line in private_lines
                                           if line.startswith(("Private_Clean:", "Private_Dirty:")))
            except (FileNotFoundError, ProcessLookupError, PermissionError):
                row["status"] = "private_unavailable"
            processes.append(row)
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            # 列挙してから読むまでに終了したプロセスや、他ユーザーの非公開情報。
            continue
    return system, processes


def runtime_snapshot(names: list[str]) -> tuple[dict, list[dict]]:
    """外部コマンドを起動せず、CPU 累積秒とメモリを読む。取得不能値は 0 と区別する。

    private_bytes は Windows では private commit、Linux では private resident。
    この違いは private_kind に残し、OS をまたぐ比較で同一指標として扱わない。
    """
    normalized = {_process_name(name) for name in names}
    if IS_WINDOWS:
        system, processes = _windows_runtime_snapshot(normalized)
    elif not IS_MACOS and Path("/proc/stat").exists():
        system, processes = _linux_runtime_snapshot(normalized)
    else:
        raise OSError("稼働計測は Windows と /proc のある Linux に対応しています")
    present = {_process_name(row["name"]) for row in processes}
    processes.extend({"name": name, "status": "missing"} for name in sorted(normalized - present))
    system.update(logical_cpus=os.cpu_count(), on_battery=on_battery())
    return system, processes
