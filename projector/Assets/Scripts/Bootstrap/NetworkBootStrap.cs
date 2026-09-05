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
        private readonly IGameMasterClient _master;
        private readonly IPacketRouter _packetRouter;
        private readonly NetworkSettings _settings;

        public NetworkBootstrap(
            IClientService<ProjectorPacket> clientService,
            IGameMasterClient masterService,
            IPacketRouter packetRouter,
            NetworkSettings settings
        )
        {
            _client = clientService ?? throw new ArgumentNullException(nameof(clientService));
            _master = masterService ?? throw new ArgumentNullException(nameof(masterService));
            _packetRouter = packetRouter ?? throw new ArgumentNullException(nameof(packetRouter));
            _settings = settings ?? throw new ArgumentNullException(nameof(settings));
        }

        internal async UniTask<NetworkConnectionResult> Initialize()
        {
            if (_client == null || _master == null || _packetRouter == null)
                return NetworkConnectionResult.InvalidConfiguration;

            Debug.Log($"[Network] {_settings}");

            _packetRouter.OnStringMessageReceived += OnReceiveMessage;


            _client.OnReceived += _packetRouter.RoutePacket;

            // 自動再接続はまだ入れていないので、せめて切れたことは分かるようにする。
            _client.ConnectionLost += OnTrackerConnectionLost;
            _master.ConnectionLost += OnMasterConnectionLost;

            _client.RegisterPort(_settings.TrackerHost, _settings.TrackerPort);

            bool isSuccessfullyClientConnect = await _client.ConnectRetryAsync(_settings.ConnectAttempts);
            if (!isSuccessfullyClientConnect) return NetworkConnectionResult.ClientConnectFailed;

            // gRPC のチャネルは遅延接続なので、ここで失敗するのはチャネルを作れないときだけ。
            // 相手が居るかどうかはイベントの購読を始めて初めて分かる。
            bool isSuccessfullyMasterConnect = await _master.ConnectAsync();
            if (!isSuccessfullyMasterConnect) return NetworkConnectionResult.MasterConnectFailed;

            return NetworkConnectionResult.Success;
        }

        private void OnTrackerConnectionLost() =>
            Debug.LogWarning("[Network] lost the connection to the tracker. Restart the projector to reconnect.");

        private void OnMasterConnectionLost() =>
            Debug.LogWarning("[Network] lost the connection to game_master. Restart the projector to reconnect.");

        private void OnReceiveMessage(TestMessage message)
        {
            Debug.Log($"Received message: {message.Message}");
            // Handle the received string message
        }

        public async UniTask DisposeAsync()
        {
            if (_client == null) return;
            _client.OnReceived -= _packetRouter.RoutePacket;
            _client.ConnectionLost -= OnTrackerConnectionLost;
            _master.ConnectionLost -= OnMasterConnectionLost;
            _packetRouter.OnStringMessageReceived -= OnReceiveMessage;

            await _client.DisconnectAsync();
            await _master.DisconnectAsync();
        }
    }
}