using Struckout.Application;
using Tk75Attractions.Struckout.V1;
using Cysharp.Threading.Tasks;
using UnityEngine;
using Struckout.Infrastructure;


namespace Struckout.Bootstrap
{
    public class NetworkBootstrap : IAsyncDestroy
    {
        private readonly IClientService<ProjectorPacket> _client;
        private readonly IClientService<MasterPacket> _master;
        private readonly IPacketRouter _packetRouter;

        public NetworkBootstrap(
            IClientService<ProjectorPacket> clientService,
            IClientService<MasterPacket> masterService,
            IPacketRouter packetRouter
        )
        {
            _client = clientService;
            _master = masterService;
            _packetRouter = packetRouter;
        }

        internal async UniTask Initialize()
        {   
            _packetRouter.OnStringMessageReceived += OnReceiveMessage;
            
            
            _client.OnReceived += _packetRouter.RoutePacket;
            _master.OnReceived += _packetRouter.RoutePacket;

            _client.RegisterPort("127.0.0.1", 5000);
            _master.RegisterPort("127.0.0.1", 5001);

            bool isSuccessfullyClientConnect = await _client.ConnectAsync();
            bool isSuccessfullyMasterConnect = await _master.ConnectAsync();

            bool isSuccessfullyConnect = isSuccessfullyClientConnect && isSuccessfullyMasterConnect;
            
            if(!isSuccessfullyConnect) throw new System.Exception("Failed to connect successfully");
        }

        private void OnReceiveMessage(TestMessage message)
        {
            Debug.Log($"Received message: {message.Message}");
            // Handle the received string message
        }

        public async UniTask DisposeAsync()
        {
            if (_client == null) return;
            _client.OnReceived -= _packetRouter.RoutePacket;
            _packetRouter.OnStringMessageReceived -= OnReceiveMessage;
            await _client.DisconnectAsync();
        }
    }
}