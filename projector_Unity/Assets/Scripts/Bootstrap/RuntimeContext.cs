using Struckout.Infrastructure.Network;
using Struckout.Application;
using System;
using System.Collections.Generic;

namespace Struckout.Bootstrap
{
    internal class RuntimeContext
    {
        public IClientService Client { get; private set; }
        public IPacketRouter PacketRouter { get; private set; }
        public List<IAsyncDestroy> DestroyEvents { get; private set; } = new();

        public RuntimeContext(
            IPacketRouter router,
            IClientService client)
        {
            PacketRouter = router;
            Client = client;
        }

        public void AddDestroyEvent(IAsyncDestroy target)
        {
            DestroyEvents.Add(target);
        }

        public void RemoveDestroyEvent(IAsyncDestroy target)
        {
            DestroyEvents.Remove(target);
        }
    }
}