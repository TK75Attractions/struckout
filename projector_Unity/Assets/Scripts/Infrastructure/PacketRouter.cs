using Tk75Attractions.Struckout.V1;

using Struckout.Application;
using System;


namespace Struckout.Infrastructure
{
    public class PacketRouter : IPacketRouter
    {
        public event Action<TestMessage> OnStringMessageReceived;
        public event Action<CollisionPoint> OnCollisionReceived;
        
        public void RoutePacket(ProjectorPacket packet)
        {
            switch (packet.PayloadCase)
            {
                case ProjectorPacket.PayloadOneofCase.Message:
                    OnStringMessageReceived?.Invoke(packet.Message);
                    break;
                case ProjectorPacket.PayloadOneofCase.Point:
                    OnCollisionReceived?.Invoke(packet.Point);
                    break;
                default:
                    UnityEngine.Debug.Log("Unknown packet type received.");
                    break;
            }
        }
    }
}