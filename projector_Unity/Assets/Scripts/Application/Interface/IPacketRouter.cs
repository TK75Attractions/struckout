using Struckout.Dto.V1;
using Struckout.Debug;
using System;

namespace Struckout.Application
{
    public interface IPacketRouter
    {
        void AddStringMessageAction(Action<StringMessage> action);
        void AddCollisionPointAction(Action<CollisionPoint> action);
        void RemoveStringMessageAction(Action<StringMessage> action);
        void RemoveCollisionPointAction(Action<CollisionPoint> action);
        void RoutePacket(NetworkPacket packet);
    }
}