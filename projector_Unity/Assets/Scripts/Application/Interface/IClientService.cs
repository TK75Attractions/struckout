using System;
using Struckout.Dto.V1;
using System.Threading.Tasks;

namespace Struckout.Application
{
    public interface IClientService
    {
        void RegisterPort(string host, int port);
        event Action<NetworkPacket> OnCollisionReceived;
        Task<bool> ConnectAsync();
        Task DisconnectAsync();
    }
}