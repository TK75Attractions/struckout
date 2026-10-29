using Tk75Attractions.Struckout.V1;
using System;
using Struckout.Application.Interface;
using Microsoft.Extensions.Logging;


namespace Struckout.Infrastructure
{
    public class PacketRouter : IPacketRouter
    {
        private readonly ILogger<PacketRouter> _logger;

        public event Action<TestMessage> OnStringMessageReceived;
        public event Action<CollisionPoint> OnCollisionReceived;


        private readonly IMainThreadDispatcher _mainThreadDispatcher;

        public PacketRouter(
            IMainThreadDispatcher dispatcher,
            ILogger<PacketRouter> logger
        )
        {
            _logger = logger ?? throw new ArgumentNullException(nameof(logger));
            _mainThreadDispatcher = dispatcher;
        }

        public void RoutePacket(ProjectorPacket packet)
        {
            if (packet == null) throw new ArgumentNullException(nameof(packet));

            switch (packet.PayloadCase)
            {
                case ProjectorPacket.PayloadOneofCase.Message:
                    if (packet.Message == null) break;
                    var message = packet.Message;
                    _mainThreadDispatcher.Enqueue(() => OnStringMessageReceived?.Invoke(message));
                    break;
                case ProjectorPacket.PayloadOneofCase.Point:
                    if (packet.Point == null) break;
                    var point = packet.Point;
                    // 購読は GameBootstrap が接続完了後に行うので、それより前に届くことがある。
                    // 購読者がいない間は捨てる (無条件に呼ぶと NullReferenceException になる)。
                    _mainThreadDispatcher.Enqueue(() => OnCollisionReceived?.Invoke(point));
                    break;
                default:
                    _logger.LogWarning("Received unknown packet type: {Type}", packet.PayloadCase);
                    break;
            }
        }

    }
}
