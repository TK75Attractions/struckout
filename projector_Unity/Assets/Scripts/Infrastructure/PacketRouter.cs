using Struckout.Dto.V1;
using Struckout.Debug;
using Struckout.Application;
using System;


namespace Struckout.Infrastructure
{
    public class PacketRouter : IPacketRouter
    {
        Action<StringMessage> _onStringMessageReceived;
        Action<CollisionPoint> _onCollisionReceived;

        public void AddStringMessageAction(Action<StringMessage> action)
        {
            _onStringMessageReceived += action;
        }

        public void AddCollisionPointAction(Action<CollisionPoint> action)
        {
            _onCollisionReceived += action;
        }

        public void RoutePacket(NetworkPacket packet)
        {
            switch (packet.PayloadCase)
            {
                case NetworkPacket.PayloadOneofCase.Message:
                    _onStringMessageReceived?.Invoke(packet.Message);
                    break;
                case NetworkPacket.PayloadOneofCase.Point:
                    _onCollisionReceived?.Invoke(packet.Point);
                    break;
                default:
                    UnityEngine.Debug.Log("Unknown packet type received.");
                    break;
            }
        }
    }
}