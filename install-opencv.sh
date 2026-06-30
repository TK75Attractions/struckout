#!/bin/sh

if [-e struckoutCameraApp]; then
	echo "プロジェクトのルートディレクトリ(install-opencv.shがあるディレクトリ)から実行してください"
	exit 1
fi
curl -OL https://github.com/opencv/opencv/releases/download/4.13.0/opencv-4.13.0-android-sdk.zip
unzip opencv-4.13.0-android-sdk.zip
mv OpenCV-android-sdk/sdk/ struckoutCameraApp/opencv
rm -rf OpenCV-android-sdk
rm opencv-4.13.0-android-sdk.zip
echo "OpenCV SDKをインストールしました"
