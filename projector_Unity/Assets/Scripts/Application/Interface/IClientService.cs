using System;
using Struckout.Dto.V1;
using System.Threading.Tasks;

namespace Struckout.Application
{
    public interface IClientService
    {
        event Action<NetworkPacket> OnCollisionReceived;
        Task<bool> ConnectAsync(string host, int port);
        Task DisconnectAsync();
    }
}