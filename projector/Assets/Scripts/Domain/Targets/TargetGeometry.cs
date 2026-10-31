namespace Struckout.Domain.TargetDefinition
{
    /// <summary>
    /// Describes a placement before registration without inventing a local entity identity.
    /// A <see cref="Target"/> is created only when game_master publishes its UUID in TargetAdded.
    /// </summary>
    public readonly struct TargetGeometry
    {
        public TargetCoordinate Coordinate { get; }
        public TargetType Type { get; }
        public float Size { get; }
        public float Diameter => Size;
        public float Radius => Size / 2f;
        public float RadiusSquared => Radius * Radius;
        public TargetVelocity Velocity { get; }

        public TargetGeometry(TargetCoordinate coordinate, TargetType type, float size, TargetVelocity velocity = default)
        {
            Coordinate = coordinate;
            Type = type;
            Size = size;
            Velocity = velocity;
        }
    }
}
