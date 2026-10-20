using Tk75Attractions.Struckout.V1;
using Cysharp.Threading.Tasks;
using UnityEngine;
using System;
using System.Threading;
using Struckout.Application.Interface;
using Struckout.Domain;


namespace Struckout.Bootstrap
{
    public class NetworkBootstrap : IAsyncDestroy
    {
        private readonly IClientService<ProjectorPacket> _client;
        private readonly IGameMasterClient _master;
        private readonly IPacketRouter _packetRouter;
        private readonly NetworkSettings _settings;

        /// <summary>ball_tracker が落ちたり再起動したりしたとき、つなぎ直す間隔。</summary>
        private static readonly TimeSpan TrackerReconnectInterval = TimeSpan.FromSeconds(2);

        private readonly CancellationTokenSource _reconnectCancellation = new();
        private int _reconnecting;

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

            // どちらも切れたらつなぎ直す (ball_tracker はここで、game_master は GrpcGameMasterClient の中で)。
            _client.ConnectionLost += OnTrackerConnectionLost;
            _master.ConnectionLost += OnMasterConnectionLost;

            _client.RegisterPort(_settings.trackerHost, _settings.trackerPort);

            bool isSuccessfullyClientConnect = await _client.ConnectRetryAsync(_settings.connectAttempts);
            if (!isSuccessfullyClientConnect) return NetworkConnectionResult.ClientConnectFailed;

            // gRPC のチャネルは遅延接続なので、ここで失敗するのはチャネルを作れないときだけ。
            // 相手が居るかどうかはイベントの購読を始めて初めて分かる。
            bool isSuccessfullyMasterConnect = await _master.ConnectAsync();
            if (!isSuccessfullyMasterConnect) return NetworkConnectionResult.MasterConnectFailed;

            return NetworkConnectionResult.Success;
        }

        private void OnTrackerConnectionLost()
        {
            Debug.LogWarning("[Network] lost the connection to the tracker. Reconnecting...");
            ReconnectTrackerAsync(_reconnectCancellation.Token).Forget();
        }

        /// <summary>
        /// ball_tracker につながるまで、一定の間隔でつなぎ直す。PC① の ball_tracker を再起動しても、
        /// projector は起動し直さなくてよい。
        /// </summary>
        private async UniTaskVoid ReconnectTrackerAsync(CancellationToken token)
        {
            if (Interlocked.Exchange(ref _reconnecting, 1) == 1) return;
            try
            {
                while (!token.IsCancellationRequested)
                {
                    try
                    {
                        if (await _client.ConnectAsync())
                        {
                            Debug.Log("[Network] reconnected to the tracker.");
                            return;
                        }
                    }
                    catch (Exception ex)
                    {
                        Debug.LogWarning($"[Network] failed to reconnect to the tracker: {ex.Message}");
                    }
                    await UniTask.Delay(TrackerReconnectInterval, cancellationToken: token);
                }
            }
            catch (OperationCanceledException)
            {
            }
            finally
            {
                Interlocked.Exchange(ref _reconnecting, 0);
            }
        }

        private void OnMasterConnectionLost() =>
            Debug.LogWarning("[Network] lost the connection to game_master. Reconnecting...");

        private void OnReceiveMessage(TestMessage message)
        {
            Debug.Log($"Received message: {message.Message}");
            // Handle the received string message
        }

        public async UniTask DisposeAsync()
        {
            _reconnectCancellation.Cancel();
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