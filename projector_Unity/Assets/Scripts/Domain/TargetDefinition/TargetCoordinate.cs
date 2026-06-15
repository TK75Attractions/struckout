namespace Struckout.Domain
{
    public struct TargetCoordinate
    {
        public float X { get; private set; }
        public float Y { get; private set; }

        public TargetCoordinate(float x, float y)
        {
            X = x;
            Y = y;
        }
    }
}