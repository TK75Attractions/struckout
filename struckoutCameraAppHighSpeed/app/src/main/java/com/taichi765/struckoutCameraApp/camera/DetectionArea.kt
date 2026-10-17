package com.taichi765.struckoutCameraApp.camera

/**
 * 検出を受け付ける範囲。この外に中心がある検出は捨てる。
 *
 * プレイヤー側のように背景が動く部分や、プロジェクターの映像が映る部分は、背景との差分では
 * ボールと区別できないので、カメラの向きで避けきれないぶんをこれで外す。
 *
 * カメラ画面のプレビューを見ながら決めるものなので、プレビューの各辺から無視する幅を、
 * その辺に垂直な方向の長さに対する百分率で持つ。プレビューは解析画像を時計回りに 90 度
 * 回したものである点に注意。
 */
data class DetectionArea(
    val leftPercent: Double = 0.0,
    val topPercent: Double = 0.0,
    val rightPercent: Double = 0.0,
    val bottomPercent: Double = 0.0,
) {
    init {
        require(isValid(leftPercent, topPercent, rightPercent, bottomPercent)) {
            "detection area must leave some of the image: $this"
        }
    }

    /**
     * 解析画像 (回転前) の座標に直した範囲。
     *
     * 解析画像の点 (x, y) は、プレビューでは (高さ - y, x) に映る。そのため、プレビューの上下が
     * 解析画像の左右に、プレビューの右左が解析画像の上下に対応する。
     */
    fun inAnalysisImage(imageWidth: Int, imageHeight: Int) = Bounds(
        left = imageWidth * topPercent / 100.0,
        top = imageHeight * rightPercent / 100.0,
        right = imageWidth * (1.0 - bottomPercent / 100.0),
        bottom = imageHeight * (1.0 - leftPercent / 100.0),
    )

    /**
     * @param x,y 解析画像 (回転前) での位置
     */
    fun contains(x: Double, y: Double, imageWidth: Int, imageHeight: Int): Boolean {
        val bounds = inAnalysisImage(imageWidth, imageHeight)
        return x >= bounds.left && x <= bounds.right && y >= bounds.top && y <= bounds.bottom
    }

    /** 画像の全体を使うか */
    val coversWholeImage: Boolean
        get() = leftPercent == 0.0 && topPercent == 0.0 && rightPercent == 0.0 && bottomPercent == 0.0

    data class Bounds(val left: Double, val top: Double, val right: Double, val bottom: Double)

    companion object {
        /**
         * 向かい合う辺の幅を足して 100 以上になると、検出を受け付ける場所が残らない。
         */
        fun isValid(
            leftPercent: Double,
            topPercent: Double,
            rightPercent: Double,
            bottomPercent: Double
        ): Boolean {
            val margins = listOf(leftPercent, topPercent, rightPercent, bottomPercent)
            return margins.all { it >= 0.0 } &&
                    leftPercent + rightPercent < 100.0 &&
                    topPercent + bottomPercent < 100.0
        }
    }
}
