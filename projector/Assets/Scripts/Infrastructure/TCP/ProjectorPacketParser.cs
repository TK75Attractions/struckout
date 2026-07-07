using Tk75Attractions.Struckout.V1;
using Struckout.Application;

namespace Struckout.Infrastructure
{
    public class ProjectorPacketParser : IMessageParser<ProjectorPacket>
    {
        public ProjectorPacket MessageParse(byte[] bytes) => ProjectorPacket.Parser.ParseFrom(bytes);
    }
}