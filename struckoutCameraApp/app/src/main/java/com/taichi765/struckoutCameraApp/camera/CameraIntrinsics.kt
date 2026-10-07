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
}
