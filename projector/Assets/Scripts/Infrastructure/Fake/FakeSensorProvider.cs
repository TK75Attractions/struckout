using System;
using Struckout.Application;
using Tk75Attractions.Struckout.V1;

namespace Struckout.Infrastructure
{
    public class FakeSensorProvider : ISensorProvider
    {
        public event Action<CollisionPoint> OnCollisionReceived;
        
        public void GetSensorData(CollisionPoint point)
        {
            OnCollisionReceived?.Invoke(point);
        }
    }
}