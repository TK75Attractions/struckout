#!/usr/bin/env bash
#
# api/proto の .proto から C# のコードを生成する。
#
# Rust は api/rust/build.rs (prost)、Kotlin は Gradle が自動生成するが、
# C# だけは protoc の出力をリポジトリにコミットしている。
# このスクリプトがその生成を担当する。手で protoc を叩いたり、
# 生成結果を別ディレクトリにコピーしたりしないこと。
#
# 生成先:
#   - projector/Assets/Scripts/ProtoBuf/Generated        (Unity)
#   - sandbox/testTcpCLI/Network/Protocol/Generated      (ダミー対向 CLI)
#
# 使い方:
#   scripts/generate-proto.sh           生成してコミット対象を更新する
#   scripts/generate-proto.sh --check   生成せず、コミット済みのファイルが
#                                       .proto と一致しているかだけを確認する。
#                                       ズレていたら一覧を表示して exit 1。
#
# Windows からは Git Bash で実行する (Git for Windows に同梱)。

set -euo pipefail

check=false
for arg in "$@"; do
    case "$arg" in
        --check) check=true ;;
        -h|--help) sed -n '2,22p' "$0" | sed 's/^#\{1,2\} \{0,1\}//'; exit 0 ;;
        *) echo "unknown argument: $arg" >&2; exit 2 ;;
    esac
done

repo_root=$(cd "$(dirname "$0")/.." && pwd)

# C# 側が必要とする .proto だけを生成する。
# struckout.proto (camera <-> tracker) と xtask_sync.proto は C# からは使わない。
proto_files=(
    "collision.proto"
    "master_and_projector.proto"
)

output_dirs=(
    "$repo_root/projector/Assets/Scripts/ProtoBuf/Generated"
    "$repo_root/sandbox/testTcpCLI/Network/Protocol/Generated"
)

# protoc がネイティブの Windows 実行ファイルのとき、bash 側のパスをそのまま渡せない。
to_native_path() {
    if command -v cygpath >/dev/null 2>&1; then
        cygpath -w "$1"
    else
        printf '%s' "$1"
    fi
}

to_bash_path() {
    if command -v cygpath >/dev/null 2>&1; then
        cygpath -u "$1"
    else
        printf '%s' "$1"
    fi
}

resolve_protoc() {
    if [ -n "${PROTOC:-}" ]; then
        local from_env
        from_env=$(to_bash_path "$PROTOC")
        if [ -x "$from_env" ]; then
            printf '%s' "$from_env"
            return
        fi
    fi

    # mise.toml pins protoc, so `mise install` puts the right version on PATH.
    if command -v protoc >/dev/null 2>&1; then
        command -v protoc
        return
    fi

    # Left over from before mise. Kept so a half-migrated checkout still works.
    local bundled
    for bundled in "$repo_root/.tools/protoc-35.1/bin/protoc.exe" \
                   "$repo_root/.tools/protoc-35.1/bin/protoc"; do
        if [ -x "$bundled" ]; then
            printf '%s' "$bundled"
            return
        fi
    done

    echo "protoc not found. Run 'mise install', or set PROTOC." >&2
    exit 1
}

protoc=$(resolve_protoc)

# protoc はネイティブ実行ファイルなので、非 ASCII を含むパスを引数で受け取れない
# (このリポジトリは "ドキュメント" 配下に置かれることがある)。
# api/proto をカレントディレクトリにして相対パスだけを渡すことで回避する。
staging=$(mktemp -d "${TMPDIR:-/tmp}/struckout-protogen-XXXXXXXX")
trap 'rm -rf "$staging"' EXIT

if printf '%s' "$staging" | LC_ALL=C grep -q '[^ -~]'; then
    echo "TEMP path contains non-ASCII characters and protoc cannot write there: $staging" >&2
    exit 1
fi

(
    cd "$repo_root/api/proto"
    "$protoc" --proto_path=. --csharp_out="$(to_native_path "$staging")" "${proto_files[@]}"
)

shopt -s nullglob
generated=("$staging"/*.cs)
shopt -u nullglob
if [ ${#generated[@]} -eq 0 ]; then
    echo "protoc produced no output" >&2
    exit 1
fi

# protoc は LF で出力し、コミット済みのファイルも LF (.gitattributes で eol=lf に
# 固定してある)。ここで改行コードを変換してはいけない。変換すると
# core.autocrlf の設定が違う環境で --check が必ず落ちる。

drifted=()

for output_dir in "${output_dirs[@]}"; do
    if [ ! -d "$output_dir" ]; then
        if $check; then
            drifted+=("missing directory: $output_dir")
            continue
        fi
        mkdir -p "$output_dir"
    fi

    for source in "${generated[@]}"; do
        destination="$output_dir/$(basename "$source")"

        if $check; then
            if [ ! -f "$destination" ]; then
                drifted+=("missing: $destination")
            elif ! cmp -s "$source" "$destination"; then
                drifted+=("out of date: $destination")
            fi
        else
            cp -f "$source" "$destination"
            echo "generated $destination"
        fi
    done
done

if $check; then
    if [ ${#drifted[@]} -gt 0 ]; then
        echo ""
        echo "Generated protobuf code is out of sync with api/proto:" >&2
        for entry in "${drifted[@]}"; do
            echo "  $entry" >&2
        done
        echo "" >&2
        echo "Run scripts/generate-proto.sh and commit the result." >&2
        exit 1
    fi
    echo "Generated protobuf code is up to date."
fi
