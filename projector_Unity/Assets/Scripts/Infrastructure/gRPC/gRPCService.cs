using Tk75Attractions.Struckout.V1;
using Struckout.Application;
using System.Threading.Tasks;
using Grpc.Core;

namespace Struckout.Infrastructure
{
    public class GRPCService : MasterToProjectorService.MasterToProjectorServiceBase, IGRPCService
    {
        public GRPCService()
        {
            
        }
        
        public override Task<StartGameResponse> StartGame(StartGameRequest request, ServerCallContext context)
        {
            return base.StartGame(request, context);
        }
    }
}