using Struckout.Dto.V1;
using Struckout.Debug;
using System;


namespace Struckout.Application
{
    public class PacketRouter
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
                    Console.WriteLine("Unknown packet type received.");
                    break;
            }
        }
    }
}