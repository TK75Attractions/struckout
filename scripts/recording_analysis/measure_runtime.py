#!/usr/bin/env python3
"""起動済みのプロセスの CPU・メモリを、外部依存なしで定期的に CSV に記録する。"""

from __future__ import annotations

import argparse
import csv
import math
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from struckout_common import runtime_snapshot, setup_console

DEFAULT_NAMES = ["ball_tracker", "ffmpeg", "StruckOut", "touchpanel", "vmmem", "vmmemWSL",
                 "com.docker.backend", "Docker Desktop"]
FIELDS = ["timestamp_utc", "elapsed_seconds", "name", "pid", "started_at", "status", "cpu_seconds",
          "rss_bytes", "private_bytes", "private_kind", "total_memory_bytes", "available_memory_bytes",
          "system_cpu_seconds", "system_idle_seconds", "logical_cpus", "on_battery"]


def positive_seconds(value: str) -> float:
    number = float(value)
    if not math.isfinite(number) or number <= 0:
        raise argparse.ArgumentTypeError("0 より大きい有限の秒数を指定してください")
    return number


def main() -> int:
    setup_console()
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seconds", type=positive_seconds, required=True, help="計測する秒数")
    parser.add_argument("--interval", type=positive_seconds, default=5.0, help="計測間隔 (秒、既定 5)")
    parser.add_argument("--out", type=Path, required=True, help="出力 CSV (既存ファイルは上書きしない)")
    parser.add_argument("--names", nargs="+", default=DEFAULT_NAMES, help="実行ファイルの名前 (.exe は省略可)")
    args = parser.parse_args()
    try:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        with args.out.open("x", encoding="utf-8", newline="") as output:
            writer = csv.DictWriter(output, fieldnames=FIELDS)
            writer.writeheader()
            started = time.monotonic()
            next_sample = started
            deadline = started + args.seconds
            while True:
                now = time.monotonic()
                if now < next_sample:
                    time.sleep(next_sample - now)
                now = time.monotonic()
                system, processes = runtime_snapshot(args.names)
                shared = {"timestamp_utc": datetime.now(timezone.utc).isoformat(),
                          "elapsed_seconds": round(now - started, 6), **system}
                for process in processes:
                    writer.writerow({**shared, **process})
                output.flush()
                if now >= deadline:
                    break
                # 遅れたサンプルをまとめて実行せず、次の時刻へ進める。
                next_sample = min(deadline, next_sample + args.interval)
                while next_sample <= time.monotonic() and next_sample < deadline:
                    next_sample = min(deadline, next_sample + args.interval)
        print(f"稼働計測を保存しました: {args.out}")
        return 0
    except (OSError, ValueError) as error:
        print(f"稼働計測に失敗しました: {error}", file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        print(f"稼働計測を中断しました (採取済みの CSV: {args.out})", file=sys.stderr)
        return 130


if __name__ == "__main__":
    raise SystemExit(main())
