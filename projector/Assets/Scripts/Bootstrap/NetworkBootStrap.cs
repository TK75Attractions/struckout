using Struckout.Application;
using Tk75Attractions.Struckout.V1;
using Cysharp.Threading.Tasks;
using UnityEngine;
using System;
using Struckout.Domain;


namespace Struckout.Bootstrap
{
    public class NetworkBootstrap : IAsyncDestroy
    {
        private readonly IClientService<ProjectorPacket> _client;
        private readonly IClientService<MasterProjectorPacket> _master;
        private readonly IPacketRouter _packetRouter;

        public NetworkBootstrap(
            IClientService<ProjectorPacket> clientService,
            IClientService<MasterProjectorPacket> masterService,
            IPacketRouter packetRouter
        )
        {
            _client = clientService ?? throw new ArgumentNullException(nameof(clientService));
            _master = masterService ?? throw new ArgumentNullException(nameof(masterService));
            _packetRouter = packetRouter ?? throw new ArgumentNullException(nameof(packetRouter));
        }

        internal async UniTask<NetworkConnectionResult> Initialize()
        {
            if (_client == null || _master == null || _packetRouter == null)
                return NetworkConnectionResult.InvalidConfiguration;

            _packetRouter.OnStringMessageReceived += OnReceiveMessage;


            _client.OnReceived += _packetRouter.RoutePacket;
            _master.OnReceived += _packetRouter.RoutePacket;

            _client.RegisterPort("127.0.0.1", 5000);
            _master.RegisterPort("172.18.208.1", 5001);

            bool isSuccessfullyClientConnect = await _client.ConnectAsync();
            if (!isSuccessfullyClientConnect) return NetworkConnectionResult.ClientConnectFailed;

            bool isSuccessfullyMasterConnect = await _master.ConnectAsync();
            if (!isSuccessfullyMasterConnect) return NetworkConnectionResult.MasterConnectFailed;

            return NetworkConnectionResult.Success;
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
            await _master.DisconnectAsync();
        }
    }
}