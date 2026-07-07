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

        private readonly IMainThreadDispatcher _mainThreadDispatcher;

        public PacketRouter(
            IMainThreadDispatcher dispatcher
        )
        {
            _mainThreadDispatcher = dispatcher;
        }
        
        public void RoutePacket(ProjectorPacket packet)
        {
            switch (packet.PayloadCase)
            {
                case ProjectorPacket.PayloadOneofCase.Message:
                    {
                        if(packet == null || packet.Message == null) break;
                        Action action = () => OnStringMessageReceived(packet.Message);
                        _mainThreadDispatcher.Enqueue(action);
                        break;
                    }
                case ProjectorPacket.PayloadOneofCase.Point:
                    {
                        if(packet == null || packet.Point == null) break;
                        Action action = () => OnCollisionReceived(packet.Point);
                        _mainThreadDispatcher.Enqueue(action);
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
                    _mainThreadDispatcher.Enqueue(action);
                    break;
            }
        }
    }
}