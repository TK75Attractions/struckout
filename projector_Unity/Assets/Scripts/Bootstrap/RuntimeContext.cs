using Struckout.Infrastructure.Network;
using Struckout.Application;
using System;
using System.Collections.Generic;

namespace Struckout.Bootstrap
{
    internal class RuntimeContext
    {
        public TCPClientService TCPClient { get; private set; } = new();
        public IPacketRouter packetRouter { get; private set; }
        public List<IAsyncDestroy> destroyEvents { get; private set; }

        public RuntimeContext(IPacketRouter router)
        {
            packetRouter = router;
        }

        public void AddDestroyEvent(IAsyncDestroy target)
        {
            destroyEvents.Add(target);
        }

        public void RemoveDestroyEvent(IAsyncDestroy target)
        {
            destroyEvents.Remove(target);
        }
    }
}