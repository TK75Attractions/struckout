using System.Collections.Generic;
using JetBrains.Annotations;
using Struckout.Application.Interface;
using Tk75Attractions.Struckout.V1;

namespace Struckout.Bootstrap
{
    public class RuntimeContext
    {
        public IClientService<ProjectorPacket> Client { get; private set; }
        public IPacketRouter PacketRouter { get; private set; }
        public List<IAsyncDestroy> DestroyEvents { get; private set; } = new();

        [UsedImplicitly(ImplicitUseKindFlags.InstantiatedNoFixedConstructorSignature)]
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