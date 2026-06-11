using StruckOut.DTO;
using StruckOut.Debug;
using System;


namespace StruckOut.Application
{
    public class PacketRouter
    {
        Action<stringMessage> _onStringMessageReceived;
        Action<CollisionPoint> _onCollisionReceived;

        public void AddStringMessageAction(Action<stringMessage> action)
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