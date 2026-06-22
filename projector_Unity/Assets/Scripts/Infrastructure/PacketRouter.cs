using Tk75Attractions.Struckout.V1;

using Struckout.Application;
using System;
using UnityEngine;


namespace Struckout.Infrastructure
{
    public class PacketRouter : IPacketRouter
    {
        public event Action<TestMessage> OnStringMessageReceived;
        public event Action<CollisionPoint> OnCollisionReceived;

        public event Action<StartGameRequest> OnGameStartReceived;

        private IMainThreadDispatcher mainThreadDispatcher;
        
        public void RoutePacket(ProjectorPacket packet)
        {
            switch (packet.PayloadCase)
            {
                case ProjectorPacket.PayloadOneofCase.Message:
                    {
                        Action action = () => OnStringMessageReceived(packet.Message);
                        mainThreadDispatcher.Enqueue(action);
                        break;
                    }
                case ProjectorPacket.PayloadOneofCase.Point:
                    {
                        Action action = () => OnCollisionReceived(packet.Point);
                        mainThreadDispatcher.Enqueue(action);
                        break;
                    }
                default:
                    Debug.Log("Unknown packet type received.");
                    break;
            }
        }

        public void RoutePacket(MasterPacket packet)
        {
            switch (packet.PayloadCase)
            {
                case MasterPacket.PayloadOneofCase.Request:
                    Action action = () => OnGameStartReceived(packet.Request);
                    mainThreadDispatcher.Enqueue(action);
                    break;
            }
        }
    }
}