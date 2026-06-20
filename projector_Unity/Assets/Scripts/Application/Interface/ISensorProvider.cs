using System;
using Tk75Attractions.Struckout.V1;

namespace Struckout.Application
{
    public interface ISensorProvider
    {
        event Action<CollisionPoint> OnCollisionReceived;
        void GetSensorData(CollisionPoint point);
    }
}