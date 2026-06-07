namespace StruckOut.Domain
{
    public struct CollisionPoint
    {
        public float X { get; private set; }
        public float Y { get; private set; }

        public CollisionPoint(float x, float y)
        {
            X = x;
            Y = y;
        }
    }
}