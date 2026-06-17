using Struckout.Application;
using Struckout.Debug;
using Cysharp.Threading.Tasks;


namespace Struckout.Bootstrap
{
    public class NetworkBootstrap : IAsyncDestroy
    {
        private readonly IClientService _Client;
        private readonly IPacketRouter _packetRouter;

        public NetworkBootstrap(
            IClientService clientService,
            IPacketRouter packetRouter
        )
        {
            _Client = clientService;
            _packetRouter = packetRouter;
        }

        internal async UniTask Initialize(RuntimeContext context)
        {   
            _packetRouter.OnStringMessageReceived += OnReceiveMessage;
            
            
            _Client.OnCollisionReceived += _packetRouter.RoutePacket;

            bool isSuccessfullyConnect = await _Client.ConnectAsync("127.0.0.1", 5000);
            if(!isSuccessfullyConnect) throw new System.Exception("Failed to connect successfully");
        }

        private void OnReceiveMessage(StringMessage message)
        {
            UnityEngine.Debug.Log($"Received message: {message.Message}");
            // Handle the received string message
        }

        public async UniTask OnDestroy()
        {
            if (_Client == null) return;
            _Client.OnCollisionReceived -= _packetRouter.RoutePacket;
            _packetRouter.OnStringMessageReceived -= OnReceiveMessage;
            await _Client.DisconnectAsync();
        }
    }
}