namespace Struckout.Domain.TargetDefinition
{
    /// <summary>Compass movement in board pixels per second, before reflection at the field edge.</summary>
    public readonly struct TargetVelocity
    {
        public double X { get; }
        public double Y { get; }

        public TargetVelocity(double x, double y)
        {
            X = x;
            Y = y;
        }
    }
}
