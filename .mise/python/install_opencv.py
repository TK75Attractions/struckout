import shutil
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


def replace_proguard(opencv_dir: Path):
    '''
       以下のエラーで落ちるのを修正する:
       `getDefaultProguardFile('proguard-android.txt')` is no longer supported since it includes `-dontoptimize`, which prevents R8 from performing many optimizations. Instead use `getDefaultProguardFile('proguard-android-optimize.txt)`, and if needed, temporarily use `-dontoptimize` in a custom keep rule file while fixing breakages. 
    '''
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
            urllib.request.urlretrieve(
                DOWNLOAD_URL,
                archive_path,
            )

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
    except (OSError, urllib.error.URLError, zipfile.BadZipFile) as error:
        print(f"OpenCV SDKのインストールに失敗しました: {error}", file=sys.stderr)
        return 1

    print("OpenCV SDKをインストールしました")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
