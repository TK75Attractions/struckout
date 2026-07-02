using System;
using System.Threading.Tasks;
using Tk75Attractions.Struckout.V1;

namespace Struckout.Application
{
    public interface IClientService<T>
    {
        void RegisterPort(string host, int port);
        event Action<T> OnReceived;
        Task<bool> ConnectAsync();
        Task DisconnectAsync();
    }
}