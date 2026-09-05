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

        public override string ToString() =>
            $"Target#{Id} ({Coordinate.X:F1}, {Coordinate.Y:F1}) d={Size:F1}";
    }
}
