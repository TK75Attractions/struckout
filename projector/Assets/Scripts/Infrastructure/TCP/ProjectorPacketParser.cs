using JetBrains.Annotations;
using Struckout.Application.Interface;
using Tk75Attractions.Struckout.V1;

namespace Struckout.Infrastructure.TCP
{
    public class ProjectorPacketParser : IMessageParser<ProjectorPacket>
    {
        [UsedImplicitly(ImplicitUseKindFlags.InstantiatedNoFixedConstructorSignature)]
        public ProjectorPacketParser()
        {
            
        }
        
        public ProjectorPacket MessageParse(byte[] bytes) => ProjectorPacket.Parser.ParseFrom(bytes);
    }
}