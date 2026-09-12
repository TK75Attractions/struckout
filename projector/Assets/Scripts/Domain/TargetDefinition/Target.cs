using System.Threading;

namespace Struckout.Domain
{
    /// <summary>
    /// 的。
    ///
    /// 当たると別の場所へ移動し、将来は動き続ける難易度も作る予定なので、
    /// 座標を等価性の一部にはしない。値が等しいかではなく「同じ的か」で扱う。
    /// 座標で等価だと、動かすたびに UI との対応づけが壊れてしまう。
    /// </summary>
    public class Target
    {
        private static int _nextId;

        /// <summary>ログで追うための通し番号。等価性は参照で決まる。</summary>
        public int Id { get; }

        public TargetCoordinate Coordinate { get; private set; }
        public TargetType Type { get; }

        /// <summary>的の直径。描画される大きさと当たり判定はこの値で一致する。</summary>
        public float Size { get; }

        public float Diameter => Size;
        public float Radius => Size / 2f;
        public float RadiusSquared => Radius * Radius;

        /// <summary>
        /// 当たり判定の対象か。移動の演出中だけ false になる。
        ///
        /// 当たった的は先に <see cref="MoveTo"/> で移動先へ移るが、画面のほうは
        /// その場で縮んで消え、移動先で膨らんで現れるまで間がある。その間ずっと
        /// 判定だけ移動先で有効だと、**何も描かれていない場所に当たり判定がある**
        /// ことになり、当たったのに何も起きないように見える。
        ///
        /// 落ち着くまで判定から外すのは <see cref="ITargetUI"/> の実装の責任。
        /// 演出の長さを知っているのは描画側だけなので、時間をここに持たせて
        /// 二重管理にはしない。
        /// </summary>
        public bool IsHittable { get; private set; } = true;

        public Target(
            TargetCoordinate coordinate,
            TargetType type,
            float size
        )
        {
            Id = Interlocked.Increment(ref _nextId);
            Coordinate = coordinate;
            Type = type;
            Size = size;
        }

        /// <summary>大きさは変えずに位置だけ移す。</summary>
        public void MoveTo(TargetCoordinate coordinate) => Coordinate = coordinate;

        /// <summary>移動の演出が始まった。見た目が落ち着くまで当たらなくなる。</summary>
        public void BeginRelocation() => IsHittable = false;

        /// <summary>見た目が移動先に現れ切った。ふたたび当たるようになる。</summary>
        public void EndRelocation() => IsHittable = true;

        public override string ToString() =>
            $"Target#{Id} ({Coordinate.X:F1}, {Coordinate.Y:F1}) d={Size:F1}" +
            (IsHittable ? "" : " relocating");
    }
}
