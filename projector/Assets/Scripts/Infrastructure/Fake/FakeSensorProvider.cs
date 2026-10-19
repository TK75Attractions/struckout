using System;
using Struckout.Application.Interface;
using Tk75Attractions.Struckout.V1;

namespace Struckout.Infrastructure.Fake
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