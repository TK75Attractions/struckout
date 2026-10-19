using System;
using Tk75Attractions.Struckout.V1;

namespace Struckout.Application.Interface
{
    public interface IPacketRouter
    {
        public event Action<TestMessage> OnStringMessageReceived;
        public event Action<CollisionPoint> OnCollisionReceived;

        void RoutePacket(ProjectorPacket packet);
    }
}