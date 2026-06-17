using Struckout.Dto.V1;
using Struckout.Debug;
using System;

namespace Struckout.Application
{
    public interface IPacketRouter
    {
        public event Action<StringMessage> OnStringMessageReceived;
        public event Action<CollisionPoint> OnCollisionReceived;
        void RoutePacket(NetworkPacket packet);
    }
}