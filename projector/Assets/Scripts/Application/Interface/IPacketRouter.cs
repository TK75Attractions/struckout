using Tk75Attractions.Struckout.V1;

using System;

namespace Struckout.Application
{
    public interface IPacketRouter
    {
        public event Action<TestMessage> OnStringMessageReceived;
        public event Action<CollisionPoint> OnCollisionReceived;

        public event Action<StartGameRequest> OnGameStartReceived;
        void RoutePacket(ProjectorPacket packet);
        void RoutePacket(MasterPacket packet);
    }
}