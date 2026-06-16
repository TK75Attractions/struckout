using System;
using Struckout.Dto.V1;
using System.Threading.Tasks;

namespace Struckout.Application
{
    public interface IClientService
    {
        void AddAction(Action<NetworkPacket> action);
        void RemoveAction(Action<NetworkPacket> action);
        Task<bool> ConnectAsync(string host, int port);
        Task DisconnectAsync();
    }
}