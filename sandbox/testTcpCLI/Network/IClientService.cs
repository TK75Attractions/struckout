using System;
using System.Threading.Tasks;
using Tk75Attractions.Struckout.V1;

namespace Struckout.Application
{
    public interface IServerService<T>
    {
        bool GetOpen();
        int GetPort();
        void RegisterPort(int port);
        Task Open();
        Task Close();
    }
}