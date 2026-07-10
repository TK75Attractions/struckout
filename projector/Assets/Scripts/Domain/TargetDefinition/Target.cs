namespace Struckout.Domain
{
    public record Target
    {
        public TargetCoordinate Coordinate { get; private set; }
        public TargetType Type { get; private set; }
        public float Size { get; private set; }

        // Size is stored as the target radius in world/UI units.
        public float Radius => Size;
        public float RadiusSquared => Size * Size;
        public float Diameter => Size * 2;

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