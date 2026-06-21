using System.Threading.Tasks;
using Grpc.Core;
using Struckout.Application;
using Tk75Attractions.Struckout.V1;

namespace Struckout.Infrastructure
{
    public class GRPCService : IGRPCService
    {
        private readonly GameRuntime _runtime;
        private readonly IMainThreadDispatcher _dispatcher;

        public GRPCService(GameRuntime runtime, IMainThreadDispatcher dispatcher)
        {
            _runtime = runtime;
            _dispatcher = dispatcher;
        }

        public Task<StartGameResponse> StartGame(StartGameRequest request, ServerCallContext context )
        {
            StartGameResponse startGameResponse = new();
            _dispatcher.Enqueue(_runtime.GameSetup);
            return Task.FromResult(startGameResponse);
        }
    }
}