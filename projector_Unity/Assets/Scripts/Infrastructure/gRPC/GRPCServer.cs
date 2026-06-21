using VContainer.Unity;
using System.Threading.Tasks;
using Grpc.Core;
using Tk75Attractions.Struckout.V1;
using System;

namespace Struckout.Infrastructure
{
    public class GRPCServer : IStartable, IAsyncDisposable
    {
        private Server server;
        private readonly GRPCServiceImpl _impl;
        private bool isInit = false;
        public GRPCServer(GRPCServiceImpl impl)
        {
            _impl = impl;
        }
        public void Start()
        {
            if (isInit) return;
            if (_impl == null) throw new Exception("The GRPCServerImpl doesn't initialized");
            server = new Server
            {
                Services = { MasterToProjectorService.BindService(_impl) },
                Ports = { new ServerPort("0.0.0.1",50051,ServerCredentials.Insecure) }
            };

            server.Start();
            isInit = true;
        }

        public async ValueTask DisposeAsync()
        {
            if (server == null) return;
            await server.ShutdownAsync();
        }
    }
}