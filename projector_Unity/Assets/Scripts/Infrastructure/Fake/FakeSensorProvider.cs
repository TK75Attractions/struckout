using System;
using Struckout.Application;
using Struckout.Dto.V1;

namespace Struckout.Infrastructure
{
    public class FakeSensorProvider : ISensorProvider
    {
        public event Action<CollisionPoint> OnCollisionReceived;
        public void GetSensorData(CollisionPoint point = null)
        {
            var points = new CollisionPoint{
                X  = 0.1f,
                Y = 0.1f
            };
            OnCollisionReceived?.Invoke(points);
        }
    }
}