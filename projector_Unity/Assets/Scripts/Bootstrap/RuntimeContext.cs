using Struckout.Infrastructure.Network;
using Struckout.Infrastructure;

namespace Struckout.Bootstrap
{
    internal class RuntimeContext
    {
        public TCPClientService _tcpClient;
        public PacketRouter packetRouter = new();
    }
}