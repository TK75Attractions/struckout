using System;

namespace Struckout.Domain
{
    /// <summary>
    /// ball_tracker から届く物理座標 (m) を、的の描画座標に変換する。
    ///
    /// 変換係数は投影の仕方 (プロジェクターの位置や台形補正) で変わる。Inspector の値は既定値で、
    /// 起動時に設定ファイル (collision_transform.json) があればその値で上書きする
    /// (<c>Struckout.Infrastructure.CollisionTransformStore</c>)。書き出したプレイヤーでも、
    /// F2 で開く調整用の画面 (<c>Struckout.Unity.CollisionCalibrationPanel</c>) から変えて保存できる。
    ///
    /// 移した先は盤面座標 (<see cref="FieldBounds"/>)。原点は左下で Y は上向き。
    /// TargetGenerator が的を置くのと同じ座標系で、ワールド座標に直すのは
    /// さらに後段の <see cref="WorldCoordinateTransform"/> の仕事。
    ///
    /// 既定値は物理 x[-1, 1] m / y[0, 2] m がちょうど既定の盤面 (1920x1080) に
    /// 収まる想定。実測に基づく値ではないので、必ず現物で合わせること。
    /// 盤面の広さを変えたときは、この係数も合わせて見直す必要がある。
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
        /// ball_tracker は CollisionPoint に、的の面に沿った横方向 (世界座標の y) と
        /// 高さ (世界座標の z) を詰めるので、通常は使わない。
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

        /// <summary>描画座標 → 物理座標 (m)。<see cref="ToRenderSpace"/> の逆。</summary>
        public (double X, double Y) ToPhysical(double renderX, double renderY)
        {
            double x = (renderX - OriginX) / PixelsPerMetreX;
            double y = (renderY - OriginY) / PixelsPerMetreY;
            if (FlipX) x = -x;
            if (FlipY) y = -y;
            if (SwapAxes)
            {
                (x, y) = (y, x);
            }

            return (x, y);
        }

        /// <summary>
        /// 盤面の 2 点 (描画座標) が、実際には物理座標 (m) のどこに映ったかから、係数を決め直す。
        /// 2 点が横にも縦にも離れていること。軸の向きが逆なら <see cref="FlipX"/> / <see cref="FlipY"/> を立てる。
        /// <see cref="SwapAxes"/> はそのまま使う。
        /// </summary>
        public void FitTwoPoints(
            (double X, double Y) render1, (double X, double Y) measured1,
            (double X, double Y) render2, (double X, double Y) measured2)
        {
            if (SwapAxes)
            {
                measured1 = (measured1.Y, measured1.X);
                measured2 = (measured2.Y, measured2.X);
            }

            var (originX, scaleX, flipX) = FitAxis(render1.X, measured1.X, render2.X, measured2.X, "x");
            var (originY, scaleY, flipY) = FitAxis(render1.Y, measured1.Y, render2.Y, measured2.Y, "y");
            OriginX = originX;
            PixelsPerMetreX = scaleX;
            FlipX = flipX;
            OriginY = originY;
            PixelsPerMetreY = scaleY;
            FlipY = flipY;
        }

        private static (float Origin, float Scale, bool Flip) FitAxis(
            double render1, double measured1, double render2, double measured2, string axis)
        {
            double span = measured2 - measured1;
            if (Math.Abs(span) < 1e-6)
            {
                throw new ArgumentException($"the two points are at the same {axis} position");
            }

            // 描画 = Origin + 符号 * 1m あたり * 物理。符号つきの傾きが負なら向きが逆
            double slope = (render2 - render1) / span;
            return ((float)(render1 - slope * measured1), (float)Math.Abs(slope), slope < 0);
        }

        /// <summary>
        /// 別のインスタンスの値を写す。同じインスタンスを持っている側 (SensorProvider など) に、
        /// 実行中の変更がそのまま効くように、インスタンスは差し替えずに値だけ写す。
        /// </summary>
        public void CopyFrom(CollisionCoordinateTransform other)
        {
            if (other == null) throw new ArgumentNullException(nameof(other));
            PixelsPerMetreX = other.PixelsPerMetreX;
            PixelsPerMetreY = other.PixelsPerMetreY;
            OriginX = other.OriginX;
            OriginY = other.OriginY;
            FlipX = other.FlipX;
            FlipY = other.FlipY;
            SwapAxes = other.SwapAxes;
            LogConversions = other.LogConversions;
        }

        public override string ToString() =>
            $"scale=({PixelsPerMetreX}, {PixelsPerMetreY}) px/m origin=({OriginX}, {OriginY}) " +
            $"flip=({FlipX}, {FlipY}) swap={SwapAxes}";
    }
}
