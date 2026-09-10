using Tk75Attractions.Struckout.V1;
using Struckout.Application;
using System;

namespace Struckout.Infrastructure
{
    public class MasterProjectorPacketParser : IMessageParser<MasterProjectorPacket>
    {
        public MasterProjectorPacket MessageParse(byte[] bytes) => MasterProjectorPacket.Parser.ParseFrom(bytes);
    }
}