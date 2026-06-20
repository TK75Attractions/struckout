using System;
using Struckout.Dto.V1;

namespace Struckout.Application
{
    public interface ISensorProvider
    {
        event Action<CollisionPoint> OnCollisionReceived;
        void GetSensorData(CollisionPoint point);
    }
}