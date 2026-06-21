using Tk75Attractions.Struckout.V1;
using Struckout.Application;
using System.Threading.Tasks;
using Grpc.Core;

namespace Struckout.Infrastructure
{
    public class GRPCServiceImpl : MasterToProjectorService.MasterToProjectorServiceBase
    {
        private readonly IGRPCService _service;
        
        public GRPCServiceImpl(IGRPCService service)
        {
            _service = service;
        }
        
        public override Task<StartGameResponse> StartGame(StartGameRequest request, ServerCallContext context)
        {
            return _service.StartGame(request, context);
        }
    }
}