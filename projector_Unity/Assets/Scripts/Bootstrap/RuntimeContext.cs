using Struckout.Infrastructure.Network;
using Struckout.Application;
using System;

namespace Struckout.Bootstrap
{
    internal class RuntimeContext
    {
        public TCPClientService TCPClient { get; private set; } = new();
        public IPacketRouter packetRouter { get; private set; }
        public Action destroyEvent { get; private set; }

        public RuntimeContext(IPacketRouter router)
        {
            packetRouter = router;
        }

        public void AddDestroyEvent(Action action)
        {
            destroyEvent += action;
        }

        public void RemoveDestroyEvent(Action action)
        {
            destroyEvent -= action;
        }
    }
}