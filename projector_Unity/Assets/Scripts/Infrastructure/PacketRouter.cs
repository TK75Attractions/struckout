using Struckout.Dto.V1;
using Struckout.Debug;
using Struckout.Application;
using System;


namespace Struckout.Infrastructure
{
    public class PacketRouter : IPacketRouter
    {
        public event Action<StringMessage> _onStringMessageReceived;
        public event Action<CollisionPoint> _onCollisionReceived;
        
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