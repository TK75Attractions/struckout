using Tk75Attractions.Struckout.V1;

using System;

namespace Struckout.Application
{
    public interface IPacketRouter
    {
        public event Action<TestMessage> OnStringMessageReceived;
        public event Action<CollisionPoint> OnCollisionReceived;
        void RoutePacket(ProjectorPacket packet);
    }
}