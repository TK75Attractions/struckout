using StruckOut.Domain;

namespace StruckOut.Infrastructure
{
    public class FakeSensorProvider : ISensorProvider
    {
        public CollisionPoint GetSensorData()
        {
            // Return fake sensor data for testing purposes
            return new CollisionPoint(0.5f, 0.5f);
        }
    }
}