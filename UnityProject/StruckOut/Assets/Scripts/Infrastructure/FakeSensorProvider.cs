using StruckOut.Domain;
using StruckOut.DTO;

namespace StruckOut.Infrastructure
{
    public class FakeSensorProvider : ISensorProvider
    {
        public CollisionPoint GetSensorData()
        {
            // Return fake sensor data for testing purposes
            return new CollisionPoint
            {
              X = 0.5f,
              Y = 0.5f  
            };
        }
    }
}