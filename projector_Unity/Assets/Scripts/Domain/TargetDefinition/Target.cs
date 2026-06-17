namespace Struckout.Domain
{
    public record Target
    {
        public TargetCoordinate Coordinate { get; private set; }
        public TargetType Type { get; private set; }
        public float Size { get; private set; }

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