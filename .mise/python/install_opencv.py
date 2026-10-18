import shutil
import subprocess
import sys
import tempfile
import urllib.error
import urllib.request
import zipfile
from pathlib import Path


OPENCV_VERSION = "4.13.0"
ARCHIVE_NAME = f"opencv-{OPENCV_VERSION}-android-sdk.zip"
DOWNLOAD_URL = (
    "https://github.com/opencv/opencv/releases/download/"
    f"{OPENCV_VERSION}/{ARCHIVE_NAME}"
)


def get_github_token() -> str | None:
    try:
        result = subprocess.run(
            ["gh", "auth", "token", "--hostname", "github.com"],
            check=True,
            capture_output=True,
            text=True,
        )
    except FileNotFoundError:
        print("ghコマンドが見つかりませんでした。認証せずにダウンロードします")
        return None
    except subprocess.CalledProcessError:
        print(
            "GitHubの認証トークンを取得できませんでした。認証せずにダウンロードします"
        )
        return None

    token = result.stdout.strip()
    if not token:
        raise RuntimeError("gh auth tokenが空のトークンを返しました")

    return token


def replace_proguard(opencv_dir: Path):
    """
    以下のエラーで落ちるのを修正する:
    `getDefaultProguardFile('proguard-android.txt')` is no longer supported since it includes `-dontoptimize`, which prevents R8 from performing many optimizations. Instead use `getDefaultProguardFile('proguard-android-optimize.txt)`, and if needed, temporarily use `-dontoptimize` in a custom keep rule file while fixing breakages.
    """
    gradle_path = opencv_dir / "build.gradle"
    content_old = gradle_path.read_text()
    content_new = content_old.replace(
        "proguard-android.txt", "proguard-android-optimize.txt"
    )
    gradle_path.write_text(content_new)


def main() -> int:
    project_root = Path.cwd()
    camera_dir = project_root / "camera"

    if not camera_dir.exists():
        print(
            "プロジェクトのルートディレクトリから実行してください",
            file=sys.stderr,
        )
        return 1

    dst = camera_dir / "opencv"
    if dst.exists():
        print(f"{dst} は既に存在します", file=sys.stderr)
        return 1

    try:
        with tempfile.TemporaryDirectory(prefix="opencv-install-") as tempdir:
            workdir = Path(tempdir)
            archive_path = workdir / ARCHIVE_NAME

            print("SDKをダウンロードしています...")
            token = get_github_token()
            headers = {"Accept": "application/octet-stream"}
            if token:
                headers["Authorization"] = f"Bearer {token}"

            request = urllib.request.Request(DOWNLOAD_URL, headers=headers)
            with urllib.request.urlopen(request) as response:
                with archive_path.open("wb") as archive_file:
                    shutil.copyfileobj(response, archive_file)

            with zipfile.ZipFile(archive_path) as archive:
                print("ダウンロードしたSDKを解凍しています...")
                archive.extractall(workdir)

            sdk_directory = workdir / "OpenCV-android-sdk" / "sdk"
            if not sdk_directory.is_dir():
                raise FileNotFoundError(
                    f"ダウンロードしたアーカイブに {sdk_directory.relative_to(workdir)} "
                    "がありません"
                )

            shutil.move(str(sdk_directory), dst)
            replace_proguard(dst)
    except (OSError, RuntimeError, urllib.error.URLError, zipfile.BadZipFile) as error:
        print(f"OpenCV SDKのインストールに失敗しました: {error}", file=sys.stderr)
        return 1

    print("OpenCV SDKをインストールしました")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
