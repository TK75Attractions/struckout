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

        public event Action<StartGame> OnGameStartReceived;

        private readonly IMainThreadDispatcher _mainThreadDispatcher;

        public PacketRouter(
            IMainThreadDispatcher dispatcher
        )
        {
            _mainThreadDispatcher = dispatcher;
        }

        public void RoutePacket(ProjectorPacket packet)
        {
            if (packet == null) return;

            switch (packet.PayloadCase)
            {
                case ProjectorPacket.PayloadOneofCase.Message:
                    {
                        if (packet.Message == null) break;
                        var message = packet.Message;
                        _mainThreadDispatcher.Enqueue(() => OnStringMessageReceived?.Invoke(message));
                        break;
                    }
                case ProjectorPacket.PayloadOneofCase.Point:
                    {
                        if (packet.Point == null) break;
                        var point = packet.Point;
                        // 購読は GameBootstrap が接続完了後に行うので、それより前に届くことがある。
                        // 購読者がいない間は捨てる (無条件に呼ぶと NullReferenceException になる)。
                        _mainThreadDispatcher.Enqueue(() => OnCollisionReceived?.Invoke(point));
                        break;
                    }
                default:
                    Debug.Log("Unknown packet type received.");
                    break;
            }
        }

        public void RoutePacket(MasterProjectorPacket packet)
        {
            if (packet == null) return;

            switch (packet.PayloadCase)
            {
                case MasterProjectorPacket.PayloadOneofCase.StartGame:
                    {
                        if (packet.StartGame == null) break;
                        var startGame = packet.StartGame;
                        _mainThreadDispatcher.Enqueue(() =>
                        {
                            if (OnGameStartReceived == null)
                            {
                                // まだ誰も StartGame を処理していない。
                                // 届いていること自体は確認できるようにログには残す。
                                Debug.Log($"[Master] StartGame({startGame.Difficulty}) received, but nothing handles it yet.");
                                return;
                            }

                            OnGameStartReceived(startGame);
                        });
                        break;
                    }
                default:
                    Debug.Log($"Unhandled master packet: {packet.PayloadCase}");
                    break;
            }
        }
    }
}
