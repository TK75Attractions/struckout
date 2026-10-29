#!/usr/bin/env python3
"""PC② をひとまとめに起動する: game_master (Docker) と、書き出した projector をプロジェクターの画面に全画面で。

    python scripts/run_pc2.py                          # 起動
    python scripts/run_pc2.py --tracker-host PC1-NAME  # つなぎ先を指定して起動
    python scripts/run_pc2.py --build                  # projector を書き出し直してから起動
    python scripts/run_pc2.py --stop                   # projector と game_master を止める

PC① の scripts/run_pc1.py と、どちらを先に実行してもよい。

1. Docker を起動し (Windows と macOS は Docker Desktop を起動する。Linux は起動しておく)、game_master とデータベースを
   起動し、データベースのテーブルを作る
2. projector の書き出し (projector/Builds/<OS>) が無いか --build なら、Unity でコマンドから書き出す
   (Unity Editor でプロジェクトを開いていると書き出せない)
3. projector を、scripts/pc2.json の設定 (つなぎ先、画面の番号、着弾位置の係数) で起動する
4. projector のログを見て、ball_tracker につながったら知らせる

projector は、ball_tracker や game_master が後から起動しても、途中で再起動しても、自分でつなぎ直す。
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import socket
import subprocess
import sys
import time
import urllib.parse
from datetime import datetime
from pathlib import Path

from struckout_common import (IS_MACOS, IS_WINDOWS, ROOT, SCRIPTS, Failure, detached_popen, fail, find_mise_tool,
                              kill_process, load_json, note, ok, per_platform, platform_key, process_running,
                              read_shared, report_failure, run, server_port, setup_console, step, tcp_ready, warn)

COMPOSE = ["compose", "-f", str(ROOT / "game_master" / "docker" / "compose.yaml"),
           "-f", str(ROOT / "game_master" / "docker" / "compose.dev.yaml")]

# Unity のコマンドラインでの書き出し先の指定 (OS ごと)
BUILD_TARGETS = {
    "windows": ("Win64", "-buildWindows64Player"),
    "linux": ("Linux64", "-buildLinux64Player"),
    "macos": ("OSXUniversal", "-buildOSXUniversalPlayer"),
}


def database_password() -> str | None:
    """データベースのパスワード。mise の MYSQL_ROOT_PASSWORD_DEV (.mise/config.local.toml、`mise run //game_master:dev`
    と同じ) を使う。環境変数に無ければ .mise/config.local.toml から読み、それも無ければ前の game_master/.env.dev から読む。
    値は表示しない。"""
    password = os.environ.get("MYSQL_ROOT_PASSWORD_DEV")
    local = ROOT / ".mise" / "config.local.toml"
    if not password and local.exists():
        match = re.search(r'^\s*MYSQL_ROOT_PASSWORD_DEV\s*=\s*"([^"]*)"', local.read_text(encoding="utf-8-sig"), re.M)
        if match:
            password = match.group(1)
    env_file = ROOT / "game_master" / ".env.dev"
    if not password and env_file.exists():
        match = re.search(r"^\s*MYSQL_ROOT_PASSWORD\s*=\s*(.*)$", env_file.read_text(encoding="utf-8-sig"), re.M)
        if match:
            password = match.group(1).strip().strip("\"'")
    return password or None


def docker(*args, env=None) -> subprocess.CompletedProcess:
    return run(["docker", *args], env=env)


def ensure_docker() -> None:
    if docker("info").returncode == 0:
        return
    if IS_WINDOWS:
        desktop = Path(os.environ.get("ProgramFiles", r"C:\Program Files")) / "Docker" / "Docker" / "Docker Desktop.exe"
        if not desktop.exists():
            fail("Docker Desktop が見つかりません")
        start = lambda: detached_popen([desktop])
    elif IS_MACOS:
        start = lambda: run(["open", "-a", "Docker"])
    else:
        fail("Docker が動いていません。sudo systemctl start docker (Docker Desktop なら systemctl --user start "
             "docker-desktop) で起動してからやり直してください")
    note("Docker Desktop を起動しています (1〜2 分)")
    start()
    deadline = time.monotonic() + 180
    relaunch = time.monotonic() + 30
    while time.monotonic() < deadline:
        time.sleep(3)
        # 起動した Docker Desktop がすぐ終わってしまうことがあるので、30 秒たっても居なければ 1 回だけ起動し直す
        if relaunch and time.monotonic() > relaunch:
            if IS_WINDOWS and not process_running("Docker Desktop.exe"):
                start()
            relaunch = None
        if docker("info").returncode == 0:
            return
    fail("Docker が起動しません。Docker Desktop を手で起動してからやり直してください")


def migrate_database(password: str) -> None:
    """データベースのテーブルを作る (作成済みなら何もしない)。game_master は起動時に作らないので、新しいデータベース
    (初めての PC、compose のプロジェクト名が変わったときなど) では、これをしないと game_master がエラーを返す。"""
    sqlx = find_mise_tool("cargo-sqlx-cli", "sqlx")
    if not sqlx:
        warn("sqlx が見つからないので、データベースのテーブルを確かめられません (mise -C game_master install cargo:sqlx-cli)。"
             "新しいデータベースだと game_master がエラーを返します")
        return
    url = f"mysql://root:{urllib.parse.quote(password, safe='')}@127.0.0.1:3306/main_dev"
    # game_master/sqlx.toml が migrations/mysql を指している
    result = run([sqlx, "migrate", "run"], cwd=ROOT / "game_master", env=dict(os.environ, DATABASE_URL=url))
    out = result.stdout.replace(password, "***").splitlines()
    if result.returncode != 0:
        fail("データベースのテーブルを作れませんでした: " + " ".join(out[-2:]))
    applied = [line for line in out if line.startswith("Applied")]
    ok(f"データベースのテーブルを作った ({len(applied)} 件)" if applied else "データベースのテーブル: 作成済み")


def start_game_master() -> None:
    step("game_master を起動する (Docker)")
    if not shutil.which("docker"):
        fail("docker が見つかりません。Docker を入れてください")
    ensure_docker()
    ok("Docker")
    password = database_password()
    if not password:
        fail("データベースのパスワードがありません。.mise/config.local.toml の [env] に MYSQL_ROOT_PASSWORD_DEV を書いてください "
             "(docs/setup_pc2.md)")
    env = dict(os.environ, MYSQL_ROOT_PASSWORD=password)
    # 2026-10-17 までの game_master は compose のプロジェクト名が game_master だった。残っていると 8020 番がぶつかるので止める
    old = docker("ps", "-q", "--filter", "name=^game_master-").stdout.split()
    if old:
        warn("前の名前 (game_master) で動いている game_master を止めます")
        docker("stop", *old)
    note("game_master を起動しています (初回はビルドに数分から十数分かかる)")
    # イメージが無いと、ここで Rust のコンパイルから組み立てる (約 20 分)。そのあいだ何も表示されないので、1 分おきに経過を出す
    up_log = Path(os.environ.get("TEMP") or "/tmp") / "struckout_game_master_up.log"
    with open(up_log, "wb") as out:
        up = subprocess.Popen(["docker", *COMPOSE, "up", "-d"], stdout=out, stderr=subprocess.STDOUT, env=env,
                              stdin=subprocess.DEVNULL)
        started = time.monotonic()
        last_note = started
        while up.poll() is None:
            time.sleep(2)
            if time.monotonic() - last_note >= 60:
                last_note = time.monotonic()
                minutes = int((time.monotonic() - started) // 60)
                building = "Running" in docker("buildx", "history", "ls").stdout
                note(f"game_master のイメージを組み立てています ({minutes} 分経過。初回は 20 分ほど)" if building
                     else f"game_master を起動しています ({minutes} 分経過)")
    for line in read_shared(up_log).splitlines():
        if re.search(r"error|Error|failed", line):
            warn(line)
    if up.returncode != 0:
        fail(f"game_master を起動できませんでした (docker compose の出力: {up_log})")
    deadline = time.monotonic() + 120
    port = server_port("game-master", "grpc")
    while not tcp_ready("127.0.0.1", port):
        if time.monotonic() > deadline:
            fail(f"game_master が {port} 番で待ち受けません (docker compose logs で確かめる)")
        time.sleep(2)
    migrate_database(password)
    ok(f"game_master: 127.0.0.1:{port}")


def unity_locked(project: Path) -> bool:
    """Unity Editor が projector を開いているか。"""
    lock = project / "Temp" / "UnityLockfile"
    if not lock.exists():
        return False
    if IS_WINDOWS:
        # Editor が開いているあいだは、ファイルの名前を変えられない
        probe = lock.with_name("UnityLockfile.probe")
        try:
            lock.rename(probe)
            probe.rename(lock)
            return False
        except OSError:
            return True
    return run(["pgrep", "-f", f"projectPath.*{re.escape(str(project))}"]).returncode == 0 or \
        run(["pgrep", "-f", "Unity.*projector"]).returncode == 0


def build_projector(settings, player: Path) -> None:
    step("projector を書き出す (数分)")
    project = ROOT / "projector"
    if unity_locked(project):
        fail("Unity Editor で projector を開いていると書き出せません。閉じてからやり直してください")
    unity = per_platform(settings["unity_editor"])
    if not unity or not Path(os.path.expanduser(unity)).exists():
        fail(f"Unity が見つかりません: {unity} (pc2.json の unity_editor の {platform_key()})")
    target, option = BUILD_TARGETS[platform_key()]
    player.parent.mkdir(parents=True, exist_ok=True)
    build_log = player.parent / "build.log"
    result = subprocess.run([os.path.expanduser(unity), "-batchmode", "-quit", "-projectPath", str(project),
                             "-buildTarget", target, option, str(player), "-logFile", str(build_log)],
                            stdin=subprocess.DEVNULL)
    if result.returncode != 0 or not player.exists():
        fail(f"書き出しに失敗しました (ログ: {build_log})")
    ok(f"書き出した: {player}")


def player_command(player: Path) -> Path:
    """書き出したものから、起動する実行ファイル。macOS は .app の中にある。"""
    if IS_MACOS and player.suffix == ".app":
        return player / "Contents" / "MacOS" / player.stem
    return player


def start_projector(settings, player: Path, tracker_host: str, windowed: bool) -> None:
    try:
        addresses = sorted({info[4][0] for info in socket.getaddrinfo(tracker_host, None, socket.AF_INET)})
        ok(f"つなぎ先: {tracker_host} ({', '.join(addresses)})")
    except OSError:
        warn(f"つなぎ先 {tracker_host} の名前が引けません。IP で指定してみてください")
    image = player_command(player).name
    if process_running(image):
        warn("projector がすでに動いていたので止めます")
        kill_process(image)
        time.sleep(1)
    log_dir = ROOT / "recordings" / (datetime.now().strftime("%Y-%m-%d") + "_projector")
    log_dir.mkdir(parents=True, exist_ok=True)

    step("projector を起動する")
    log = log_dir / f"projector_{datetime.now().strftime('%H%M%S')}.log"
    # -connectAttempts を大きくして、ball_tracker が起動するまで待ち続けてもらう (間隔は 5 秒まで)
    args = ["-trackerHost", tracker_host, "-masterHost", settings["master_host"], "-machineId", settings["machine_id"],
            "-connectAttempts", 100000, "-monitor", settings["monitor"], "-logFile", log,
            "-screen-fullscreen", 0 if windowed or not settings["fullscreen"] else 1]
    if settings.get("collision_transform"):
        args += ["-collisionTransform", ROOT / settings["collision_transform"]]
    projector = detached_popen([player_command(player), *args], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    ok(f"projector を起動した (ログ: {log})")

    step("PC① の ball_tracker につながるのを待つ (Ctrl+C でやめても projector は待ち続ける)")
    deadline = time.monotonic() + 600
    last_note = time.monotonic()
    connected = False
    while not connected and time.monotonic() < deadline:
        time.sleep(1)
        if projector.poll() is not None:
            fail(f"projector が終了しました (ログ: {log})")
        text = read_shared(log)
        if "Master connect failed" in text:
            fail("game_master につながりません (docker compose ps で確かめる)")
        if "Connected to TCP server" in text:
            # ball_tracker の次に game_master へつなぐので、少し待ってから決める
            time.sleep(5)
            if "Master connect failed" in read_shared(log):
                fail("game_master につながりません (docker compose ps で確かめる)")
            connected = True
        elif time.monotonic() - last_note >= 30:
            note("ball_tracker を待っています (PC① で run_pc1.py を実行する)")
            last_note = time.monotonic()
    if connected:
        ok("ball_tracker と game_master につながった")
    else:
        warn("ball_tracker にまだつながりません。projector は待ち続けます。つなぎ先 (pc2.json の tracker_host) と "
             "PC① のファイアウォールを確かめてください")


def main() -> int:
    setup_console()
    parser = argparse.ArgumentParser(description="PC② をひとまとめに起動する")
    parser.add_argument("--tracker-host", help="pc2.json の tracker_host (PC① の名前か IP) を上書きする")
    parser.add_argument("--build", action="store_true", help="projector を書き出し直す")
    parser.add_argument("--windowed", action="store_true", help="全画面にしない (動作の確かめ用)")
    parser.add_argument("--stop", action="store_true", help="projector と game_master を止める")
    args = parser.parse_args()

    settings = load_json(SCRIPTS / "pc2.json")
    player_path = per_platform(settings["projector_exe"])
    if not player_path:
        print(f"pc2.json の projector_exe に {platform_key()} の値がありません")
        return 1
    player = ROOT / player_path
    try:
        if args.stop:
            step("止める")
            kill_process(player_command(player).name)
            ok("projector")
            if settings.get("game_master"):
                password = database_password()
                env = dict(os.environ, MYSQL_ROOT_PASSWORD=password) if password else None
                docker(*COMPOSE, "stop", env=env)
                ok("game_master")
            return 0

        tracker_host = args.tracker_host or settings.get("tracker_host")
        if not tracker_host:
            fail("つなぎ先 (PC①) が決まっていません。--tracker-host <PC① の名前か IP> を付けるか、scripts/pc2.json の "
                 "tracker_host に書いてください")
        if settings.get("game_master"):
            start_game_master()
        if args.build or not player.exists():
            build_projector(settings, player)
        start_projector(settings, player, tracker_host, args.windowed)
        print()
        print("止めるとき: python scripts/run_pc2.py --stop")
        return 0
    except Failure as e:
        return report_failure(e)
    except KeyboardInterrupt:
        print()
        return 130


if __name__ == "__main__":
    sys.exit(main())
