using Struckout.Dto.V1;
using Struckout.Debug;
using System;

namespace Struckout.Application
{
    public interface IPacketRouter
    {
        public event Action<StringMessage> _onStringMessageReceived;
        public event Action<CollisionPoint> _onCollisionReceived;
        void RoutePacket(NetworkPacket packet);
    }
}