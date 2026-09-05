namespace Struckout.Domain
{
    public record Target
    {
        public TargetCoordinate Coordinate { get; private set; }
        public TargetType Type { get; private set; }
        /// <summary>的の直径。描画される大きさと当たり判定はこの値で一致する。</summary>
        public float Size { get; private set; }

        public float Diameter => Size;
        public float Radius => Size / 2f;
        public float RadiusSquared => Radius * Radius;

        public Target(
            TargetCoordinate coordinate,
            TargetType type,
            float size
        )
        {
            Coordinate = coordinate;
            Type = type;
            Size = size;
        }

    }
}