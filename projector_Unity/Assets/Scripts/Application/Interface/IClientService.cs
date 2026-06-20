using System;
using Tk75Attractions.Struckout.V1;
using System.Threading.Tasks;

namespace Struckout.Application
{
    public interface IClientService
    {
        void RegisterPort(string host, int port);
        event Action<ProjectorPacket> OnCollisionReceived;
        Task<bool> ConnectAsync();
        Task DisconnectAsync();
    }
}