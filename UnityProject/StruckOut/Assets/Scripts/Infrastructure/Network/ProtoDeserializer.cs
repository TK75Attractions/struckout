using StruckOut.DTO;
using Google.Protobuf;
using System;

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
    }
}