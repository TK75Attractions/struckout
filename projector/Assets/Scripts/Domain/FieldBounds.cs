using System;

namespace Struckout.Domain
{
    /// <summary>
    /// 的を置ける盤面の矩形。単位はピクセル。
    ///
    /// これはプロジェクタの解像度ではなく、投影する絵の論理的な広さ。
    /// 実際の投影面が 16:9 でなくても、盤面はこの比率のまま保たれる
    /// (<see cref="WorldCoordinateTransform"/> が余白を作って全体を映す)。
    ///
    /// 一箇所にまとめてあるのは、盤面の広さを前提にしている場所が3つあるため。
    /// TargetGenerator が的を置く範囲、CollisionCoordinateTransform が
    /// 物理座標を移す先、WorldCoordinateTransform が描画に移す元。
    /// ここが分かれていると、比率を変えたときに三者が食い違う。
    /// </summary>
    [Serializable]
    public class FieldBounds
    {
        public float Width = 1920f;
        public float Height = 1080f;

        public FieldBounds() { }

        public FieldBounds(float width, float height)
        {
            Width = width;
            Height = height;
        }

        public float CentreX => Width / 2f;
        public float CentreY => Height / 2f;

        /// <summary>盤面の縦横比。0 除算を避けるため高さが 0 以下なら 1 を返す。</summary>
        public float Aspect => Height <= 0f ? 1f : Width / Height;

        public bool Contains(float x, float y) =>
            x >= 0f && x <= Width && y >= 0f && y <= Height;

        public override string ToString() => $"field={Width}x{Height}";
    }
}
