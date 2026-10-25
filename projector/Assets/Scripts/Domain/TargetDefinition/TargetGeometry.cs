namespace Struckout.Domain.TargetDefinition
{
    /// <summary>
    /// Describes a placement before registration without inventing a local entity identity.
    /// A Target is created only after game_master supplies its UUID.
    /// </summary>
    public readonly struct TargetGeometry
    {
        public TargetCoordinate Coordinate { get; }
        public TargetType Type { get; }
        public float Size { get; }
        public float Diameter => Size;
        public float Radius => Size / 2f;
        public float RadiusSquared => Radius * Radius;

        public TargetGeometry(TargetCoordinate coordinate, TargetType type, float size)
        {
            Coordinate = coordinate;
            Type = type;
            Size = size;
        }
    }
}