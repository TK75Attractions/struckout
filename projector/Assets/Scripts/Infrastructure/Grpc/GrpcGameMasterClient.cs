using System;
using System.Threading;
using System.Threading.Tasks;
using Cysharp.Net.Http;
using Grpc.Core;
using Grpc.Net.Client;
using Struckout.Application;
using Struckout.Domain;
using Tk75Attractions.Struckout.V1;
using UnityEngine;

// UnityEngine.Event と名前が衝突するので、proto 側を明示する。
using Event = Tk75Attractions.Struckout.V1.Event;

namespace Struckout.Infrastructure
{
    /// <summary>
    /// game_master (tonic) への gRPC クライアント。
    ///
    /// Unity の HttpClient は HTTP/2 を話せないので、トランスポートは
    /// YetAnotherHttpHandler を使う。TLS を張らないので Http2Only = true にして
    /// h2c (平文の HTTP/2) で接続する。
    /// </summary>
    public class GrpcGameMasterClient : IGameMasterClient
    {
        private readonly NetworkSettings _settings;

        private YetAnotherHttpHandler _handler;
        private GrpcChannel _channel;
        private GameMasterService.GameMasterServiceClient _client;

        private CancellationTokenSource _listenCancellation;
        private Task _listenTask;

        /// <summary>進行中のゲーム。StartGame を受け取るまでは無い。</summary>
        private int? _gameId;

        public event Action<Event.Types.GameStarted> GameStarted;
        public event Action ConnectionLost;

        public GrpcGameMasterClient(NetworkSettings settings)
        {
            _settings = settings ?? throw new ArgumentNullException(nameof(settings));
        }

        public Task<bool> ConnectAsync()
        {
            if (_channel != null) return Task.FromResult(true);

            var address = $"http://{_settings.MasterHost}:{_settings.MasterPort}";

            try
            {
                _handler = new YetAnotherHttpHandler { Http2Only = true };
                _channel = GrpcChannel.ForAddress(address, new GrpcChannelOptions
                {
                    HttpHandler = _handler,
                    DisposeHttpClient = true,
                });
                _client = new GameMasterService.GameMasterServiceClient(_channel);
            }
            catch (Exception ex)
            {
                Debug.LogError($"[GameMaster] failed to open a channel to {address}: {ex.Message}");
                Cleanup();
                return Task.FromResult(false);
            }

            _listenCancellation = new CancellationTokenSource();
            _listenTask = ListenEventsAsync(_listenCancellation.Token);

            Debug.Log($"[GameMaster] listening for events on {address}");
            return Task.FromResult(true);
        }

        /// <summary>
        /// イベントの購読。ここが唯一 game_master の状態を知る経路になる。
        ///
        /// 接続できるかどうかはこのストリームを読み始めて初めて分かる。
        /// gRPC のチャネルは遅延接続なので、ConnectAsync の時点では失敗しない。
        /// </summary>
        private async Task ListenEventsAsync(CancellationToken token)
        {
            try
            {
                using var call = _client.ListenEvents(new ListenEventsRequest(), cancellationToken: token);

                while (await call.ResponseStream.MoveNext(token))
                {
                    HandleEvent(call.ResponseStream.Current);
                }

                Debug.LogWarning("[GameMaster] the event stream ended.");
            }
            catch (OperationCanceledException)
            {
                // 自分で止めた。
                return;
            }
            catch (RpcException ex)
            {
                Debug.LogWarning($"[GameMaster] the event stream failed: {ex.Status.StatusCode} {ex.Status.Detail}");
            }
            catch (Exception ex)
            {
                Debug.LogException(ex);
            }

            try
            {
                ConnectionLost?.Invoke();
            }
            catch (Exception ex)
            {
                Debug.LogException(ex);
            }
        }

        private void HandleEvent(Event received)
        {
            switch (received.DataCase)
            {
                case Event.DataOneofCase.GameStarted:
                    var started = received.GameStarted;

                    // ListenEvents に絞り込みが無いので、全台のイベントが流れてくる。
                    // 自分の号機のものだけを拾う。
                    if (started.MachineId != _settings.MachineId)
                    {
                        Debug.Log($"[GameMaster] ignored an event for machine {started.MachineId}");
                        return;
                    }

                    _gameId = started.GameId;
                    GameStarted?.Invoke(started);
                    break;

                default:
                    // 終了や残り時間はまだ proto に無い。増えたらここに足す。
                    Debug.Log($"[GameMaster] unhandled event: {received.DataCase}");
                    break;
            }
        }

        public async Task<bool> AddScoreAsync(int scoreToAdd)
        {
            if (_client == null)
            {
                Debug.LogWarning("[GameMaster] not connected; the score was not sent.");
                return false;
            }

            if (_gameId is not int gameId)
            {
                Debug.LogWarning("[GameMaster] no game is running; the score was not sent.");
                return false;
            }

            try
            {
                await _client.AddScoreAsync(new AddScoreRequest
                {
                    MachineId = _settings.MachineId,
                    GameId = gameId,
                    ScoreToAdd = scoreToAdd,
                });
                return true;
            }
            catch (RpcException ex)
            {
                Debug.LogWarning($"[GameMaster] AddScore failed: {ex.Status.StatusCode} {ex.Status.Detail}");
                return false;
            }
        }

        public async Task DisconnectAsync()
        {
            if (_channel == null) return;

            _listenCancellation?.Cancel();

            if (_listenTask != null)
            {
                try
                {
                    await _listenTask;
                }
                catch (Exception ex)
                {
                    Debug.LogWarning($"[GameMaster] the event stream ended with {ex.GetType().Name}");
                }
            }

            Cleanup();
            Debug.Log("[GameMaster] disconnected");
        }

        private void Cleanup()
        {
            _listenCancellation?.Dispose();
            _listenCancellation = null;
            _listenTask = null;

            _channel?.Dispose();
            _channel = null;

            _handler?.Dispose();
            _handler = null;

            _client = null;
            _gameId = null;
        }
    }
}
