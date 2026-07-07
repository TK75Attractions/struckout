using Tk75Attractions.Struckout.V1;
using Struckout.Application;

namespace Struckout.Infrastructure
{
    public class MasterPacketParser : IMessageParser<MasterPacket>
    {
        public MasterPacket MessageParse(byte[] bytes) => MasterPacket.Parser.ParseFrom(bytes);
    }
}