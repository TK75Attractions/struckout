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

        public PacketRouter(
            IMainThreadDispatcher dispatcher
        )
        {
            mainThreadDispatcher = dispatcher;
        }
        
        public void RoutePacket(ProjectorPacket packet)
        {
            switch (packet.PayloadCase)
            {
                case ProjectorPacket.PayloadOneofCase.Message:
                    {
                        if(packet == null || packet.Message == null) break;
                        Action action = () => OnStringMessageReceived(packet.Message);
                        mainThreadDispatcher.Enqueue(action);
                        break;
                    }
                case ProjectorPacket.PayloadOneofCase.Point:
                    {
                        if(packet == null || packet.Point == null) break;
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