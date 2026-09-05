using System;

namespace Struckout.Domain
{
    /// <summary>
    /// ball_tracker から届く物理座標 (m) を、的の描画座標に変換する。
    ///
    /// 変換係数はまだ決まっていない (盤面の実寸と、ball_tracker が縦に使う軸が未確定)。
    /// そのため係数はここに切り出して Inspector から調整できるようにしてある。
    /// 実測して値が固まったら、そのまま Inspector の値を確定させればよい。
    ///
    /// 描画側は RectTransform.anchoredPosition なので Y は上向き。
    /// TargetGenerator は X を 0〜1920、Y を 0〜1080 で置いている。
    ///
    /// 既定値は物理 x[-1, 1] m / y[0, 2] m がちょうど 1920x1080 に収まる想定。
    /// 実測に基づく値ではないので、必ず現物で合わせること。
    /// </summary>
    [Serializable]
    public class CollisionCoordinateTransform
    {
        /// <summary>1 m あたりの描画座標。</summary>
        public float PixelsPerMetreX = 960f;
        public float PixelsPerMetreY = 540f;

        /// <summary>物理座標の原点 (0, 0) が描画座標のどこに来るか。</summary>
        public float OriginX = 960f;
        public float OriginY = 0f;

        /// <summary>軸の向きが逆だったとき用。</summary>
        public bool FlipX = false;
        public bool FlipY = false;

        /// <summary>
        /// 縦横が入れ替わっていたとき用。
        /// ball_tracker は CollisionPoint に (x, z) を詰めているが、
        /// そこに「これあってる?」の FIXME が残っているため逃げ道を用意してある。
        /// </summary>
        public bool SwapAxes = false;

        /// <summary>変換前後の値をログに出す。係数を合わせ込むとき用。</summary>
        public bool LogConversions = false;

        /// <summary>物理座標 (m) → 描画座標。</summary>
        public (double X, double Y) ToRenderSpace(double physicalX, double physicalY)
        {
            if (SwapAxes)
            {
                (physicalX, physicalY) = (physicalY, physicalX);
            }

            double x = OriginX + (FlipX ? -physicalX : physicalX) * PixelsPerMetreX;
            double y = OriginY + (FlipY ? -physicalY : physicalY) * PixelsPerMetreY;

            return (x, y);
        }

        public override string ToString() =>
            $"scale=({PixelsPerMetreX}, {PixelsPerMetreY}) px/m origin=({OriginX}, {OriginY}) " +
            $"flip=({FlipX}, {FlipY}) swap={SwapAxes}";
    }
}
