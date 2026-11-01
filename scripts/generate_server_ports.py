"""Generate port constants for modules that cannot read servers.yaml at build time."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SPEC_PATH = REPOSITORY_ROOT / "api" / "spec" / "servers.yaml"
PROJECTOR_OUTPUT_PATH = (
    REPOSITORY_ROOT
    / "projector"
    / "Assets"
    / "Scripts"
    / "Domain"
    / "ServerPorts.Generated.cs"
)
COMPOSE_OUTPUT_PATH = REPOSITORY_ROOT / "game_master" / "docker" / "compose.yaml"
COMPOSE_BEGIN = "# BEGIN GENERATED SERVER PORTS"
COMPOSE_END = "# END GENERATED SERVER PORTS"


class ServerPortsError(ValueError):
    """Raised when the server port specification is missing or malformed."""


def load_server_ports(spec_path: Path | None = None) -> dict[str, dict[str, int]]:
    """Read module endpoint ports from servers.yaml, rejecting unsupported YAML forms."""
    path = spec_path or DEFAULT_SPEC_PATH
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except (OSError, UnicodeError) as error:
        raise ServerPortsError(f"servers.yaml を読み込めません: {path}: {error}") from error

    if not lines or lines[0] != "servers:":
        raise ServerPortsError(f"servers.yaml の先頭に servers: がありません: {path}")

    result: dict[str, dict[str, int]] = {}
    current_module: str | None = None
    current_endpoint: str | None = None
    endpoint_type: str | None = None
    endpoint_port: int | None = None

    def finish_endpoint(line_number: int) -> None:
        nonlocal endpoint_type, endpoint_port
        if current_endpoint is None:
            return
        if endpoint_type not in {"grpc", "tcp"} or endpoint_port is None:
            raise ServerPortsError(
                f"servers.yaml の {line_number} 行目: "
                f"{current_module}.{current_endpoint} に type または port がありません"
            )
        if current_module is None:
            raise ServerPortsError(f"servers.yaml の {line_number} 行目: module の外に endpoint があります")
        result[current_module][current_endpoint] = endpoint_port
        endpoint_type = None
        endpoint_port = None

    for line_number, line in enumerate(lines[1:], start=2):
        if not line.strip() or line.startswith("#"):
            continue

        if match := re.fullmatch(r"  ([a-z][a-z0-9-]*):", line):
            finish_endpoint(line_number)
            module = match.group(1)
            if module in result:
                raise ServerPortsError(f"servers.yaml の {line_number} 行目: {module} が重複しています")
            result[module] = {}
            current_module = module
            current_endpoint = None
            continue

        if match := re.fullmatch(r"    ([a-z][a-z0-9-]*):", line):
            finish_endpoint(line_number)
            if current_module is None:
                raise ServerPortsError(f"servers.yaml の {line_number} 行目: module の外に endpoint があります")
            endpoint = match.group(1)
            if endpoint in result[current_module]:
                raise ServerPortsError(
                    f"servers.yaml の {line_number} 行目: {current_module}.{endpoint} が重複しています"
                )
            current_endpoint = endpoint
            continue

        if match := re.fullmatch(r"      (type|port): ([^\s#]+)", line):
            if current_module is None or current_endpoint is None:
                raise ServerPortsError(f"servers.yaml の {line_number} 行目: endpoint の外に値があります")
            key, value = match.groups()
            if key == "type":
                if endpoint_type is not None:
                    raise ServerPortsError(
                        f"servers.yaml の {line_number} 行目: "
                        f"{current_module}.{current_endpoint}.type が重複しています"
                    )
                endpoint_type = value
            else:
                if endpoint_port is not None:
                    raise ServerPortsError(
                        f"servers.yaml の {line_number} 行目: "
                        f"{current_module}.{current_endpoint}.port が重複しています"
                    )
                if not value.isascii() or not value.isdecimal():
                    raise ServerPortsError(f"servers.yaml の {line_number} 行目: port が整数ではありません")
                port = int(value)
                if not 1 <= port <= 65535:
                    raise ServerPortsError(f"servers.yaml の {line_number} 行目: port が範囲外です")
                endpoint_port = port
            continue

        raise ServerPortsError(f"servers.yaml の {line_number} 行目: 対応していない形式です: {line}")

    finish_endpoint(len(lines) + 1)
    if not result or any(not endpoints for endpoints in result.values()):
        raise ServerPortsError(f"servers.yaml に port 定義がありません: {path}")
    return result


def get_server_port(
    module_name: str,
    endpoint: str,
    spec_path: Path | None = None,
) -> int:
    """Return a single port from servers.yaml or raise ServerPortsError."""
    ports = load_server_ports(spec_path)
    try:
        return ports[module_name][endpoint]
    except KeyError as error:
        raise ServerPortsError(f"servers.yaml に {module_name}.{endpoint} がありません") from error


def _constant_name(module_name: str, endpoint: str, separator: str) -> str:
    name = "_".join(part for part in re.split(r"[^a-zA-Z0-9]+", f"{module_name}_{endpoint}") if part)
    if not name or name[0].isdigit():
        raise ServerPortsError(f"定数名にできない server 名です: {module_name}.{endpoint}")
    if separator == "_":
        return name.upper()
    return "".join(part.title() for part in name.split("_"))


def _projector_source(ports: dict[str, dict[str, int]]) -> str:
    constants = [
        f"        public const int {_constant_name(module, endpoint, '')} = {port};"
        for module, endpoints in ports.items()
        for endpoint, port in endpoints.items()
    ]
    return "\n".join(
        [
            "// Generated by scripts/generate_server_ports.py from api/spec/servers.yaml.",
            "namespace Struckout.Domain",
            "{",
            "    public static class ServerPorts",
            "    {",
            *constants,
            "    }",
            "}",
            "",
        ]
    )


def _camera_source(ports: dict[str, dict[str, int]]) -> str:
    constants = [
        f"    const val {_constant_name(module, endpoint, '_')} = {port}"
        for module, endpoints in ports.items()
        for endpoint, port in endpoints.items()
    ]
    return "\n".join(
        [
            "// Generated by scripts/generate_server_ports.py from api/spec/servers.yaml.",
            "package com.taichi765.struckoutCameraApp.network",
            "",
            "internal object ServerPorts {",
            *constants,
            "}",
            "",
        ]
    )


def _compose_source(ports: dict[str, dict[str, int]], indent: str) -> str:
    port = _required_port(ports, "game-master", "grpc")
    return "\n".join(
        [
            f"{indent}{COMPOSE_BEGIN}",
            f'{indent}- "{port}:{port}"',
            f"{indent}{COMPOSE_END}",
        ]
    )


def _required_port(ports: dict[str, dict[str, int]], module: str, endpoint: str) -> int:
    try:
        return ports[module][endpoint]
    except KeyError as error:
        raise ServerPortsError(f"servers.yaml に {module}.{endpoint} がありません") from error


def _update_compose(text: str, ports: dict[str, dict[str, int]]) -> str:
    lines = text.splitlines()
    starts = [index for index, line in enumerate(lines) if line.strip() == COMPOSE_BEGIN]
    ends = [index for index, line in enumerate(lines) if line.strip() == COMPOSE_END]
    if len(starts) != 1 or len(ends) != 1 or starts[0] >= ends[0]:
        raise ServerPortsError("compose.yaml にserver port生成マーカーが1組必要です")
    indent = re.match(r"[ \t]*", lines[starts[0]]).group(0)
    block = _compose_source(ports, indent).splitlines()
    lines[starts[0] : ends[0] + 1] = block
    return "\n".join(lines) + "\n"


def _write_or_check(path: Path, expected: str, check: bool) -> bool:
    try:
        current = path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        if check:
            print(f"生成ファイルを読めません: {path}: {error}", file=sys.stderr)
            return False
        current = ""
    if current == expected:
        return True
    if check:
        print(f"生成ファイルが古いです: {path}", file=sys.stderr)
        return False
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        path.write_text(expected, encoding="utf-8", newline="\n")
    except OSError as error:
        print(f"生成ファイルを書き込めません: {path}: {error}", file=sys.stderr)
        return False
    return True


def main() -> int:
    parser = argparse.ArgumentParser(description="servers.yamlから各module用のport定数を生成する")
    parser.add_argument("--check", action="store_true", help="生成済みのC#とComposeの差分を確認する")
    parser.add_argument("--camera-output", type=Path, help="Gradle用のKotlin定数を出力するdirectory")
    parser.add_argument(
        "--ports",
        nargs="+",
        metavar="VALUE",
        help="moduleと1つ以上のendpointを指定し、port一覧を空白区切りで表示する",
    )
    args = parser.parse_args()

    try:
        ports = load_server_ports()
        if args.ports is not None:
            if args.check or args.camera_output is not None or len(args.ports) < 2:
                raise ServerPortsError("--ports には module と1つ以上の endpoint のみを指定してください")
            module, *endpoints = args.ports
            print(" ".join(str(_required_port(ports, module, endpoint)) for endpoint in endpoints))
            return 0

        if args.camera_output is not None:
            if args.check:
                raise ServerPortsError("--camera-output と --check は同時に指定できません")
            camera_path = (
                args.camera_output
                / "com"
                / "taichi765"
                / "struckoutCameraApp"
                / "network"
                / "ServerPorts.kt"
            )
            return 0 if _write_or_check(camera_path, _camera_source(ports), False) else 1

        compose = _update_compose(COMPOSE_OUTPUT_PATH.read_text(encoding="utf-8"), ports)
        projector = _projector_source(ports)
        valid = _write_or_check(PROJECTOR_OUTPUT_PATH, projector, args.check)
        valid = _write_or_check(COMPOSE_OUTPUT_PATH, compose, args.check) and valid
    except (OSError, UnicodeError, ServerPortsError) as error:
        print(f"port定数の生成に失敗しました: {error}", file=sys.stderr)
        return 1

    return 0 if valid else 1


if __name__ == "__main__":
    raise SystemExit(main())
