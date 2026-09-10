#!/usr/bin/env bash
#
# grpc_csharp_plugin を .tools/bin/ に用意する。
#
# C# の gRPC スタブは protoc だけでは作れず、このプラグインが要る。
# プラグインは Grpc.Tools NuGet にしか同梱されておらず、単体配布も mise の
# バックエンドも無いので、必要なときだけ nupkg を取ってきて展開する。
#
# buf は buf.gen.game-master.yaml から `local: grpc_csharp_plugin` として
# 呼ぶ。.tools/bin は .mise/config.toml の _.path で PATH に載るため、
# 実行ファイルの拡張子 (.exe) を設定に書かずに済む。
#
# 使い方:
#   scripts/fetch-grpc-csharp-plugin.sh          無ければ取得する
#   scripts/fetch-grpc-csharp-plugin.sh --force  取得しなおす

set -euo pipefail

force=false
for arg in "$@"; do
    case "$arg" in
        --force) force=true ;;
        -h|--help) sed -n '2,15p' "$0" | sed 's/^#\{1,2\} \{0,1\}//'; exit 0 ;;
        *) echo "unknown argument: $arg" >&2; exit 2 ;;
    esac
done

repo_root=$(cd "$(dirname "$0")/.." && pwd)

# projector/Assets/packages.config の Grpc.* と揃えること。
# 生成されたスタブは同じ系列のランタイムと対で動く。
grpc_tools_version="2.61.0"

platform=""
exe=""
case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) platform="windows_x64"; exe=".exe" ;;
    Darwin)
        case "$(uname -m)" in
            arm64) platform="macosx_arm64" ;;
            *)     platform="macosx_x64" ;;
        esac
        ;;
    *)
        case "$(uname -m)" in
            aarch64|arm64) platform="linux_arm64" ;;
            *)             platform="linux_x64" ;;
        esac
        ;;
esac

bin_dir="$repo_root/.tools/bin"
destination="$bin_dir/grpc_csharp_plugin$exe"

if [ -x "$destination" ] && ! $force; then
    echo "grpc_csharp_plugin is already there: $destination"
    exit 0
fi

cache_dir="$repo_root/.tools/grpc-tools-$grpc_tools_version"
plugin="$cache_dir/tools/$platform/grpc_csharp_plugin$exe"

if [ ! -x "$plugin" ] || $force; then
    echo "downloading Grpc.Tools $grpc_tools_version for $platform ..." >&2
    archive="$cache_dir.nupkg"
    mkdir -p "$cache_dir"
    curl -fsSL -o "$archive" \
        "https://www.nuget.org/api/v2/package/Grpc.Tools/$grpc_tools_version"
    unzip -qo "$archive" "tools/$platform/*" -d "$cache_dir"
    rm -f "$archive"
    chmod +x "$plugin" 2>/dev/null || true
fi

if [ ! -x "$plugin" ]; then
    echo "grpc_csharp_plugin not found at $plugin" >&2
    exit 1
fi

mkdir -p "$bin_dir"
cp -f "$plugin" "$destination"
chmod +x "$destination" 2>/dev/null || true
echo "installed $destination"
