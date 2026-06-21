using Struckout.Application;
using Tk75Attractions.Struckout.V1;
using Cysharp.Threading.Tasks;
using UnityEngine;
using Struckout.Infrastructure;


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

            _Client.RegisterPort("127.0.0.1", 5000);
            bool isSuccessfullyConnect = await _Client.ConnectAsync();
            
            if(!isSuccessfullyConnect) throw new System.Exception("Failed to connect successfully");
        }

        private void OnReceiveMessage(TestMessage message)
        {
            Debug.Log($"Received message: {message.Message}");
            // Handle the received string message
        }

        public async UniTask DisposeAsync()
        {
            if (_Client == null) return;
            _Client.OnCollisionReceived -= _packetRouter.RoutePacket;
            _packetRouter.OnStringMessageReceived -= OnReceiveMessage;
            await _Client.DisconnectAsync();
        }
    }
}