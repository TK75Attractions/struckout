using Struckout.Infrastructure.Network;
using Struckout.Infrastructure;
using System;

namespace Struckout.Bootstrap
{
    internal class RuntimeContext
    {
        public TCPClientService TCPClient { get; private set; } = new();
        public PacketRouter packetRouter { get; private set; } = new();
        public Action destroyEvent { get; private set; }

        public void AddDestoryEvent(Action action)
        {
            destroyEvent += action;
        }

        public void RemoveDestroyEvent(Action action)
        {
            destroyEvent -= action;
        }
    }
}