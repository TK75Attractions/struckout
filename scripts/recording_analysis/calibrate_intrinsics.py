#!/usr/bin/env python3
"""チェッカーボードを撮った画像から、焦点距離・光学中心・レンズの歪み (1 項) を求める。

    python calibrate_intrinsics.py <画像のフォルダ> --board 9x6 --out intrinsics.json --debug-dir debug/

カメラアプリの「Save frame」で保存した解析画像 (センサーの向きのまま) を 15 枚ほど使う。端末ごと、
アプリ (撮影モード) ごとに 1 回行い、表示された比をアプリの設定画面の Focal length に入れる。

必要なもの: Python 3、numpy、opencv-python。撮り方は README.md を参照。
"""
import argparse
import json
import sys
from pathlib import Path

import cv2
import numpy as np

MIN_IMAGES = 5   # これより少ないと、焦点距離と光学中心を分けて決められない


def read_image(path):
    """画像を BGR で読む。cv2.imread は日本語の入ったパスを読めないことがあるので使わない。"""
    return cv2.imdecode(np.fromfile(str(path), np.uint8), cv2.IMREAD_COLOR)


def find_corners(gray, pattern):
    """チェッカーボードの交点を探す。見つからなければ None。

    解析画像 (640 px 幅) のように小さい画像は、2 倍に拡大してから探す。合成の画像で比べたところ、
    そのほうが交点の位置のずれが小さかった (二乗平均 0.16 px → 0.07 px)。見つからなければ、もう一方の大きさでも探す。
    """
    for zoom in ((2, 1) if gray.shape[1] < 1280 else (1, 2)):
        source = gray if zoom == 1 else cv2.resize(gray, None, fx=zoom, fy=zoom, interpolation=cv2.INTER_CUBIC)
        found, corners = cv2.findChessboardCornersSB(
            source, pattern, flags=cv2.CALIB_CB_NORMALIZE_IMAGE | cv2.CALIB_CB_EXHAUSTIVE | cv2.CALIB_CB_ACCURACY)
        if not found:
            found, corners = cv2.findChessboardCorners(
                source, pattern, flags=cv2.CALIB_CB_ADAPTIVE_THRESH | cv2.CALIB_CB_NORMALIZE_IMAGE)
            if found:
                corners = cv2.cornerSubPix(source, corners, (5, 5), (-1, -1),
                                           (cv2.TERM_CRITERIA_EPS + cv2.TERM_CRITERIA_MAX_ITER, 40, 0.01))
        if found:
            return (corners.reshape(-1, 2).astype(np.float64) + 0.5) / zoom - 0.5
    return None


def calibrate(object_points, image_points, size):
    """OpenCV で較正する。焦点距離は縦横で同じ値、歪みは 1 項 (k1) だけにする。"""
    flags = cv2.CALIB_FIX_ASPECT_RATIO | cv2.CALIB_ZERO_TANGENT_DIST | cv2.CALIB_FIX_K2 | cv2.CALIB_FIX_K3
    rms, matrix, distortion, _, _, deviations, _, per_view = cv2.calibrateCameraExtended(
        object_points, image_points, size, np.eye(3), np.zeros(5), flags=flags)
    deviations = deviations.ravel()
    return {
        # 縦横の焦点距離を同じにすると、OpenCV は確からしさを縦 (fy) の側にだけ返す
        "focal_px": float(matrix[0, 0]), "focal_sigma_px": float(max(deviations[0], deviations[1])),
        "center_px": [float(matrix[0, 2]), float(matrix[1, 2])],
        "center_sigma_px": [float(deviations[2]), float(deviations[3])],
        "k1": float(distortion.ravel()[0]), "k1_sigma": float(deviations[4]),
        "rms_px": float(rms), "per_image_rms_px": [float(v) for v in per_view.ravel()],
    }


def pinhole_for_app(model, size):
    """アプリの前提 (光学中心は画像の中央、歪みなし) で、視線の誤差が画像全体で最も小さくなる焦点距離を求める。

    較正した値 (光学中心、歪みつき) で画像の各点の視線を求め、アプリの式で求めた視線と比べる。
    カメラの向きは別に較正するので、視線全体が一定の角度だけ回る分は誤差に数えない。
    焦点距離と、そのとき残る視線の誤差 (px に直した二乗平均と最大) を返す。
    """
    width, height = size
    us, vs = np.meshgrid(np.arange(4.0, width, 8.0), np.arange(4.0, height, 8.0))
    pixels = np.column_stack([us.ravel(), vs.ravel()])
    distorted = (pixels - model["center_px"]) / model["focal_px"]
    normalized = distorted.copy()
    for _ in range(50):
        normalized = distorted / (1.0 + model["k1"] * (normalized ** 2).sum(axis=1, keepdims=True))
    true = np.column_stack([normalized, np.ones(len(pixels))])
    true /= np.linalg.norm(true, axis=1, keepdims=True)

    def errors(focal):
        app = np.column_stack([(pixels - [width / 2.0, height / 2.0]) / focal, np.ones(len(pixels))])
        app /= np.linalg.norm(app, axis=1, keepdims=True)
        # app の視線をまとめて回して true に最も近づける回転 (Kabsch)
        u, _, vt = np.linalg.svd(true.T @ app)
        rotation = u @ np.diag([1.0, 1.0, np.linalg.det(u @ vt)]) @ vt
        return np.linalg.norm(true - app @ rotation.T, axis=1) * focal

    best = model["focal_px"]
    for span in (0.2, 0.002):
        candidates = best * np.linspace(1.0 - span, 1.0 + span, 401)
        best = float(min(candidates, key=lambda focal: (errors(focal) ** 2).sum()))
    remaining = errors(best)
    return best, float(np.sqrt((remaining ** 2).mean())), float(remaining.max())


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("folder", type=Path, help="チェッカーボードを撮った画像 (png, jpg) のフォルダ")
    parser.add_argument("--board", required=True, help="交点の数 (マスの数ではない)。横x縦。10x7 マスなら 9x6")
    parser.add_argument("--out", type=Path, help="結果の JSON")
    parser.add_argument("--debug-dir", type=Path, help="見つけた交点を描き込んだ画像の保存先")
    args = parser.parse_args()

    try:
        columns, rows = (int(v) for v in args.board.lower().split("x"))
    except ValueError:
        sys.exit("--board は 9x6 のように書いてください")
    paths = sorted(p for p in args.folder.iterdir() if p.suffix.lower() in (".png", ".jpg", ".jpeg"))
    if not paths:
        sys.exit(f"{args.folder} に画像がありません")
    if args.debug_dir:
        args.debug_dir.mkdir(parents=True, exist_ok=True)

    # マスの大きさは結果に影響しない (焦点距離は px、歪みは無次元) ので、1 マスを 1 として置く。
    board = np.zeros((columns * rows, 3), np.float32)
    board[:, :2] = np.mgrid[:columns, :rows].T.reshape(-1, 2)
    size, used, image_points, missed = None, [], [], []
    for path in paths:
        image = read_image(path)
        if image is None:
            missed.append(path.name)
            continue
        if size is None:
            size = (image.shape[1], image.shape[0])
        if (image.shape[1], image.shape[0]) != size:
            print(f"  {path.name}: 大きさが {image.shape[1]}x{image.shape[0]} で、ほかと違うので使いません")
            continue
        corners = find_corners(cv2.cvtColor(image, cv2.COLOR_BGR2GRAY), (columns, rows))
        if corners is None:
            missed.append(path.name)
            continue
        used.append(path.name)
        image_points.append(corners.astype(np.float32).reshape(-1, 1, 2))
        if args.debug_dir:
            cv2.drawChessboardCorners(image, (columns, rows), image_points[-1], True)
            ok, encoded = cv2.imencode(".jpg", image)
            encoded.tofile(str(args.debug_dir / (path.stem + ".jpg")))
    if size is None:
        sys.exit(f"{args.folder} の画像を読めません")
    print(f"画像 {len(paths)} 枚のうち {len(used)} 枚で交点が見つかりました ({size[0]}x{size[1]})")
    if missed:
        print(f"  見つからなかった画像: {', '.join(missed)}")
    if len(used) < MIN_IMAGES:
        sys.exit(f"交点の見つかった画像が {MIN_IMAGES} 枚に足りません。--board の数 (交点の数) と、ボードが全部映っているかを"
                 "確かめてください")
    width, height = size
    every = np.concatenate(image_points).reshape(-1, 2)
    cells = set(zip((every[:, 0] * 8 // width).astype(int), (every[:, 1] * 8 // height).astype(int)))
    print(f"交点は画像の {100 * len(cells) / 64:.0f}% に散らばっています (縦横 8 つずつに分けた区画のうち、交点が入った割合)。"
          "隅まで散らばっていないと、歪みと光学中心は決まりません")

    model = calibrate([board] * len(used), image_points, size)
    offset = [model["center_px"][0] - width / 2.0, model["center_px"][1] - height / 2.0]
    print("\n較正した値")
    print(f"  焦点距離   {model['focal_px']:.2f} px (確からしさ {model['focal_sigma_px']:.2f} px)。画像の幅の "
          f"{model['focal_px'] / width:.5f} 倍")
    print(f"  光学中心   {model['center_px'][0]:.1f}, {model['center_px'][1]:.1f} px (画像の中央から {offset[0]:+.1f}, "
          f"{offset[1]:+.1f} px)")
    corner = np.hypot(width, height) / 2.0 / model["focal_px"]
    at_corner = model["focal_px"] * model["k1"] * corner ** 3
    print(f"  歪み k1    {model['k1']:+.4f} (確からしさ {model['k1_sigma']:.4f})。画像の隅で {at_corner:+.1f} px、"
          f"中心から隅までの半分の距離で {at_corner / 8:+.2f} px (負は中心へ縮む向き)")
    print(f"  残差       {model['rms_px']:.2f} px (交点の位置の二乗平均)")

    app_focal, remaining_rms, remaining_max = pinhole_for_app(model, size)
    print(f"\n設定画面の Focal length に入れる値: {app_focal / width:.5f}   ({width} px 幅で {app_focal:.2f} px)")
    print("  アプリは光学中心を画像の中央とし、歪みを扱わない。その前提で、視線の誤差が画像全体で最も小さくなる焦点距離。")
    print(f"  この値を使っても残る視線の誤差: 二乗平均 {remaining_rms:.2f} px、最大 {remaining_max:.2f} px "
          "(向きの較正で吸収される一定の回転は除いた値)")

    # アプリが画像と同じ名前で保存する JSON があれば、アプリが使っていた焦点距離と比べる。
    info_path = (args.folder / used[0]).with_suffix(".json")
    used_by_app = None
    if info_path.exists():
        try:
            used_by_app = float(json.loads(info_path.read_text(encoding="utf-8"))["fx"])
        except (ValueError, KeyError, TypeError):
            used_by_app = None
    if used_by_app:
        print(f"  アプリが使っていた値は {used_by_app:.2f} px (比 {used_by_app / width:.5f})。求めた値はそれより "
              f"{100 * (app_focal / used_by_app - 1):+.2f} %")

    limit = max(2.5 * float(np.median(model["per_image_rms_px"])), 0.3)
    outliers = [f"{name} ({rms:.2f} px)" for name, rms in zip(used, model["per_image_rms_px"]) if rms > limit]
    if outliers:
        print(f"残差の大きい画像 (ぶれやボードの反りがないか確かめる): {', '.join(outliers)}")

    if args.out:
        result = {
            "image_size": [width, height], "board": [columns, rows], "images": used, "images_without_board": missed,
            "coverage": len(cells) / 64, **model, "distortion_at_corner_px": float(at_corner),
            "app": {"focal_px": app_focal, "focal_ratio": app_focal / width, "ray_error_rms_px": remaining_rms,
                    "ray_error_max_px": remaining_max, "focal_used_by_app_px": used_by_app},
        }
        args.out.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
        print(f"結果を {args.out} に書き出しました")


if __name__ == "__main__":
    main()
