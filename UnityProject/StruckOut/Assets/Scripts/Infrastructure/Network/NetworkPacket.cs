namespace StruckOut.DTO
{
    public class NetworkPacket
    {
        public byte[] Data { get; private set; }
        public MessageType Type { get; private set; }

        public NetworkPacket(byte[] data, MessageType type)
        {
            Data = data;
            Type = type;
        }
    }
}