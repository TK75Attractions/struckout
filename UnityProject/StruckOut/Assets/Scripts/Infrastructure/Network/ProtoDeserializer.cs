using StruckOut.DTO;
using Google.Protobuf;
using System;
using StruckOut.Debug;

namespace StruckOut.Infrastructure
{
    public class ProtoDeserializer
    {
        public CollisionPoint Deserialize(byte[] data)
        {
            try
            {
                return CollisionPoint.Parser.ParseFrom(data);
            }
            catch (Exception ex)
            {
                // Log the exception or handle it as needed
                throw new InvalidOperationException("Failed to deserialize CollisionPoint", ex);
            }
        }

        public stringMessage DeserializeStringMessage(byte[] data)
        {
            try
            {
                return stringMessage.Parser.ParseFrom(data);
            }
            catch (Exception ex)
            {
                // Log the exception or handle it as needed
                throw new InvalidOperationException("Failed to deserialize CollisionPoint", ex);
            }
        }
    }
}