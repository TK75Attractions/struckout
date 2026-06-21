using VContainer.Unity;
using Grpc.Core;
using Tk75Attractions.Struckout.V1;

namespace Struckout.Infrastructure
{
    public class GRPCServer : IStartable
    {
        private Server server;
        private readonly GRPCServiceImpl impl;
        public void Start()
        {
            server = new Server
            {
                Services = { MasterToProjectorService.BindService(impl) },
                Ports = { new ServerPort("0.0.0.1",50051,ServerCredentials.Insecure) }
            };

            server.Start();
        }
    }
}