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

        /// <summary>進行中のゲーム。GameStarted を受け取るまでは無い。</summary>
        private uint? _gameId;

        public event Action<Difficulty> GameStarted;
        public event Action GameFinished;
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

            Debug.Log($"[GameMaster] listening for events on {address} as machine {_settings.MachineId}");
            return Task.FromResult(true);
        }

        /// <summary>
        /// イベントの購読。ここが唯一 game_master の状態を知る経路になる。
        ///
        /// 接続できるかどうかはこのストリームを読み始めて初めて分かる。
        /// gRPC のチャネルは遅延接続なので、ConnectAsync の時点では失敗しない。
        ///
        /// machine_id での絞り込みは game_master 側が行う
        /// (service.rs の pass_events_through_by_machine_id)。
        /// 号機番号を間違えると、繋がってはいるのにイベントが一件も来ない。
        /// </summary>
        private async Task ListenEventsAsync(CancellationToken token)
        {
            try
            {
                var request = new ListenEventsRequest { MachineId = (uint)_settings.MachineId };
                using var call = _client.ListenEvents(request, cancellationToken: token);

                while (await call.ResponseStream.MoveNext(token))
                {
                    var received = call.ResponseStream.Current?.Event;
                    if (received == null)
                    {
                        Debug.LogWarning("[GameMaster] received a response without an event; ignored.");
                        continue;
                    }

                    HandleEvent(received);
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
            switch (received.EventDataCase)
            {
                case Event.EventDataOneofCase.GameStarted:
                    // game_id は Event 直下にある。得点を送るのに要るのはこれだけ。
                    _gameId = received.GameId;
                    GameStarted?.Invoke(received.GameStarted.Difficulty);
                    break;

                case Event.EventDataOneofCase.GameFinished:
                    // これ以降 game_master は running_games から外すので、
                    // 得点を送っても NotFound で弾かれる。手元でも送信を止める。
                    _gameId = null;
                    GameFinished?.Invoke();
                    break;

                case Event.EventDataOneofCase.GameTimeLimitNotify:
                    // 毎秒届く。残り時間を projector と touchpanel のどちらが出すかが
                    // 未決なので、今は捨てている。projector で出すと決まったら
                    // IGameMasterClient にイベントを足してここから流す。
                    break;

                default:
                    Debug.Log($"[GameMaster] unhandled event: {received.EventDataCase}");
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

            if (_gameId is not uint gameId)
            {
                Debug.LogWarning("[GameMaster] no game is running; the score was not sent.");
                return false;
            }

            if (scoreToAdd < 0)
            {
                Debug.LogWarning($"[GameMaster] refused to send a negative score delta ({scoreToAdd}).");
                return false;
            }

            try
            {
                await _client.AddScoreAsync(new AddScoreRequest
                {
                    MachineId = (uint)_settings.MachineId,
                    GameId = gameId,
                    ScoreToAdd = (uint)scoreToAdd,
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
