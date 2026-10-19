package com.taichi765.struckoutCameraApp.camera

import kotlin.math.max

/**
 * ピンホールカメラの内部パラメータ。単位はピクセル。
 *
 * どの画像の座標系で測った値かは持たない。センサー座標系の値は [scaledToImage] で
 * 解析画像の座標系に直してから使うこと。
 */
data class CameraIntrinsics(val fx: Double, val fy: Double, val cx: Double, val cy: Double) {
    /**
     * センサーの有効画素領域 (active array) の座標系で表された値を、
     * そこから作られた [imageWidth] x [imageHeight] の画像の座標系に直す。
     *
     * Camera2 の出力ストリームは active array の中央を出力の縦横比で切り出して
     * 拡大縮小したものなので、その対応を当てはめる。デジタルズームや手ぶれ補正で
     * 切り出し位置が変わる場合は成り立たない。
     */
    fun scaledToImage(
        activeArrayWidth: Int,
        activeArrayHeight: Int,
        imageWidth: Int,
        imageHeight: Int
    ): CameraIntrinsics {
        require(activeArrayWidth > 0 && activeArrayHeight > 0) { "active array size must be positive" }
        require(imageWidth > 0 && imageHeight > 0) { "image size must be positive" }

        val scale = max(
            imageWidth.toDouble() / activeArrayWidth,
            imageHeight.toDouble() / activeArrayHeight
        )
        val cropX = (activeArrayWidth * scale - imageWidth) / 2
        val cropY = (activeArrayHeight * scale - imageHeight) / 2
        return CameraIntrinsics(
            fx = fx * scale,
            fy = fy * scale,
            cx = cx * scale - cropX,
            cy = cy * scale - cropY
        )
    }

    companion object {
        /**
         * LENS_INTRINSIC_CALIBRATION を返さない端末向けに、焦点距離とセンサー寸法から
         * active array 座標系での値を見積もる。
         *
         * 光学中心は active array の中央にあるものとし、レンズの歪みは考えない。
         * 実測の較正値ほどの精度は出ない。
         *
         * @param physicalWidthMm,physicalHeightMm pixel array 全体の物理寸法 (SENSOR_INFO_PHYSICAL_SIZE)
         */
        fun fromSensorInfo(
            focalLengthMm: Float,
            physicalWidthMm: Float,
            physicalHeightMm: Float,
            pixelArrayWidth: Int,
            pixelArrayHeight: Int,
            activeArrayWidth: Int,
            activeArrayHeight: Int
        ): CameraIntrinsics {
            require(focalLengthMm > 0f) { "focal length must be positive" }
            require(physicalWidthMm > 0f && physicalHeightMm > 0f) { "physical sensor size must be positive" }
            require(pixelArrayWidth > 0 && pixelArrayHeight > 0) { "pixel array size must be positive" }

            return CameraIntrinsics(
                fx = focalLengthMm.toDouble() * pixelArrayWidth / physicalWidthMm,
                fy = focalLengthMm.toDouble() * pixelArrayHeight / physicalHeightMm,
                cx = activeArrayWidth / 2.0,
                cy = activeArrayHeight / 2.0
            )
        }
    }
}

/**
 * LENS_POSE_ROTATION を返さない端末向けの、背面カメラの標準的な向き。
 *
 * 端末センサー座標 (x: 右、y: 上、z: 画面の手前) からカメラ座標 (x: 画像の右、y: 画像の下、
 * z: 光軸) への回転行列を行優先で返す。LENS_POSE_ROTATION が表す回転と同じ向き。
 *
 * カメラは端末の真後ろを向き、画像は [sensorOrientationDegrees] だけ時計回りに回すと
 * 正立する、という前提から導いている。実際の取り付け誤差は含まないので、
 * ずれは設定画面の設置角度で吸収すること。
 *
 * @param sensorOrientationDegrees SENSOR_ORIENTATION。90 の倍数
 */
fun defaultBackCameraRotation(sensorOrientationDegrees: Int): DoubleArray {
    val (cos, sin) = when (((sensorOrientationDegrees % 360) + 360) % 360) {
        0 -> 1.0 to 0.0
        90 -> 0.0 to 1.0
        180 -> -1.0 to 0.0
        270 -> 0.0 to -1.0
        else -> throw IllegalArgumentException("sensor orientation must be a multiple of 90: $sensorOrientationDegrees")
    }
    return doubleArrayOf(
        cos, -sin, 0.0,
        -sin, -cos, 0.0,
        0.0, 0.0, -1.0
    )
}
