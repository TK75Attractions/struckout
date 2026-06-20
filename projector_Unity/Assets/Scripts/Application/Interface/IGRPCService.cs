using Tk75Attractions.Struckout.V1;
using Grpc.Core;
using System.Threading.Tasks;


namespace Struckout.Application
{
    public interface IGRPCService
    {
        public Task<StartGameResponse> StartGame(StartGameRequest request, ServerCallContext context);
    }
}