#! /usr/bin/env bash
# カメラアプリの「Save frame」で保存した較正用の画像を、接続中の全端末から取り込む。
#
#   scripts/pull-calibration-frames.sh [保存先のフォルダ]
#
# 保存先 (既定は recordings/calibration) の下に、端末のシリアルごとのフォルダを作る。
# 30fps 版と 120fps 版はアプリの ID が違うので、両方を見る。
dest="${1:-recordings/calibration}"
packages="com.taichi765.struckoutCameraApp com.taichi765.struckoutCameraApp.highspeed"

adb devices |
  awk 'NR > 1 && $2 == "device" {print $1}' |
  while IFS= read -r serial; do
    echo "Using device: $serial"
    for package in $packages; do
      remote="/sdcard/Android/data/$package/files/calibration"
      # adb が次の端末のシリアルを読み取ってしまわないよう、標準入力を切る。
      if adb -s "$serial" shell "ls $remote" < /dev/null > /dev/null 2>&1; then
        mkdir -p "$dest/$serial"
        # 末尾の /. で、フォルダの中身だけを取り込む。
        MSYS_NO_PATHCONV=1 adb -s "$serial" pull "$remote/." "$dest/$serial" < /dev/null || exit 1
      fi
    done
  done
