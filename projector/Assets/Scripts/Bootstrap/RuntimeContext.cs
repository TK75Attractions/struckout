using Struckout.Application;
using System.Collections.Generic;
using Tk75Attractions.Struckout.V1;
using UnityEngine;

namespace Struckout.Bootstrap
{
    public class RuntimeContext
    {
        public IClientService<ProjectorPacket> Client { get; private set; }
        public IPacketRouter PacketRouter { get; private set; }
        public List<IAsyncDestroy> DestroyEvents { get; private set; } = new();

        public RuntimeContext(
            IPacketRouter router,
            IClientService<ProjectorPacket> client
        )
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