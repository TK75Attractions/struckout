using System;
using System.Threading;
using System.Threading.Tasks;
using Cysharp.Net.Http;
using Grpc.Core;
using Grpc.Net.Client;
using JetBrains.Annotations;
using Struckout.Application.Interface;
using Struckout.Domain;
using Tk75Attractions.Struckout.GameMaster.V1;
using Struckout.Domain.TargetDefinition;
using UnityEngine;
// UnityEngine.Event と名前が衝突するので、proto 側を明示する。
using Event = Tk75Attractions.Struckout.GameMaster.V1.Event;
using DomainTarget = Struckout.Domain.TargetDefinition.Target;
using ProtoTarget = Tk75Attractions.Struckout.GameMaster.V1.Target;

namespace Struckout.Infrastructure.Grpc
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
        private GameMasterSessionService.GameMasterSessionServiceClient _client;

        private CancellationTokenSource _listenCancellation;
        private Task _listenTask;

        /// <summary>machine_id に対応するゲームの開始・終了通知から管理する。</summary>
        private volatile bool _running;

        /// <summary>game_master と切れたとき、購読をやり直す間隔。</summary>
        private static readonly TimeSpan ReconnectInterval = TimeSpan.FromSeconds(2);

        public event Action<Difficulty> GameStarted;
        public event Action GameFinished;
        public event Action<TimeSpan> RemainingTimeChanged;
        public event Action<uint> ScoreChanged;
        public event Action ConnectionLost;
        public event Action<DomainTarget> TargetAdded;
        public event Action<Guid> TargetRemoved;

        [UsedImplicitly(ImplicitUseKindFlags.InstantiatedNoFixedConstructorSignature)]
        public GrpcGameMasterClient(NetworkSettings settings)
        {
            _settings = settings ?? throw new ArgumentNullException(nameof(settings));
        }

        public Task<bool> ConnectAsync()
        {
            if (_channel != null) return Task.FromResult(true);

            var address = $"http://{_settings.masterHost}:{_settings.masterPort}";

            try
            {
                _handler = new YetAnotherHttpHandler { Http2Only = true };
                _channel = GrpcChannel.ForAddress(address, new GrpcChannelOptions
                {
                    HttpHandler = _handler,
                    DisposeHttpClient = true,
                });
                _client = new GameMasterSessionService.GameMasterSessionServiceClient(_channel);
            }
            catch (Exception ex)
            {
                Debug.LogError($"[GameMaster] failed to open a channel to {address}: {ex.Message}");
                Cleanup();
                return Task.FromResult(false);
            }

            _listenCancellation = new CancellationTokenSource();
            _listenTask = ListenEventsAsync(_listenCancellation.Token);

            Debug.Log($"[GameMaster] listening for events on {address} as machine {_settings.machineId}");
            return Task.FromResult(true);
        }

        /// <summary>
        /// イベントの購読。ここが唯一 game_master の状態を知る経路になる。
        ///
        /// 接続できるかどうかはこのストリームを読み始めて初めて分かる。
        /// gRPC のチャネルは遅延接続なので、ConnectAsync の時点では失敗しない。
        ///
        /// machine_id での絞り込みは game_master 側が行う
        /// (services/mod.rs)。
        /// 号機番号を間違えると、繋がってはいるのにイベントが一件も来ない。
        ///
        /// 切れたら (game_master の再起動、まだ起動していないなど) <see cref="ReconnectInterval"/> おきに
        /// 購読をやり直す。projector を起動し直さなくてよい。切れたことは最初の 1 回だけ知らせる。
        /// 切断時は命中の送信を止める。状態の再取得 API がないため、
        /// 再接続後は次の GameStarted を待つ。
        /// </summary>
        private async Task ListenEventsAsync(CancellationToken token)
        {
            var lost = false;
            while (!token.IsCancellationRequested)
            {
                try
                {
                    var request = new ListenEventsRequest { MachineId = _settings.machineId };
                    using var call = _client.ListenEvents(request, cancellationToken: token);

                    // 応答のヘッダーが届いたら、つながっている。イベントはゲームが動くまで来ない
                    await call.ResponseHeadersAsync;
                    if (lost)
                    {
                        Debug.Log("[GameMaster] reconnected; listening for events again.");
                        lost = false;
                    }

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
                catch (OperationCanceledException) when (token.IsCancellationRequested)
                {
                    // 自分で止めた。
                    return;
                }
                catch (RpcException ex) when (token.IsCancellationRequested && ex.StatusCode == StatusCode.Cancelled)
                {
                    return;
                }
                catch (RpcException ex)
                {
                    if (!lost) Debug.LogWarning($"[GameMaster] the event stream failed: {ex.Status.StatusCode} {ex.Status.Detail}");
                }
                catch (Exception ex)
                {
                    if (!lost) Debug.LogException(ex);
                }

                if (!lost)
                {
                    lost = true;
                    _running = false;
                    try
                    {
                        ConnectionLost?.Invoke();
                    }
                    catch (Exception ex)
                    {
                        Debug.LogException(ex);
                    }
                }

                try
                {
                    await Task.Delay(ReconnectInterval, token);
                }
                catch (OperationCanceledException)
                {
                    return;
                }
            }
        }

        private void HandleEvent(Event received)
        {
            switch (received.EventDataCase)
            {
                case Event.EventDataOneofCase.GameStarted:
                    _running = true;
                    GameStarted?.Invoke(received.GameStarted.Difficulty);
                    break;

                case Event.EventDataOneofCase.GameFinished:
                    if (!_running) break;
                    _running = false;
                    GameFinished?.Invoke();
                    break;

                case Event.EventDataOneofCase.GameTimeLimitNotify:
                    if (!_running) break;
                    var remaining = received.GameTimeLimitNotify.Remaining;
                    if (remaining == null) break;

                    TimeSpan remainingTime;
                    try
                    {
                        remainingTime = remaining.ToTimeSpan();
                    }
                    catch (InvalidOperationException)
                    {
                        Debug.LogWarning("[GameMaster] received an invalid remaining duration; ignored.");
                        break;
                    }
                    RemainingTimeChanged?.Invoke(remainingTime);
                    break;

                case Event.EventDataOneofCase.GameScoreChanged:
                    if (!_running) break;
                    ScoreChanged?.Invoke(received.GameScoreChanged.TotalScore);
                    break;

                case Event.EventDataOneofCase.TargetAdded:
                    var added = received.TargetAdded;
                    var data = added.Target;
                    if (data == null || !Guid.TryParse(added.TargetId, out var targetId) || targetId == Guid.Empty)
                    {
                        Debug.LogWarning("[GameMaster] 的の追加通知に有効な UUID または的データがありません。");
                        break;
                    }
                    if (data.MachineId != _settings.machineId) break;
                    var compass = data.Compass;
                    if (compass == null || float.IsNaN(compass.X) || float.IsInfinity(compass.X) ||
                        float.IsNaN(compass.Y) || float.IsInfinity(compass.Y) ||
                        float.IsNaN(compass.Radius) || compass.Radius <= 0 || float.IsInfinity(compass.Radius * 2f))
                    {
                        Debug.LogWarning("[GameMaster] 的の追加通知に有効な円の形状がありません。");
                        break;
                    }
                    TargetAdded?.Invoke(new DomainTarget(targetId,
                        new TargetGeometry(new TargetCoordinate(compass.X, compass.Y), TargetType.Circle, compass.Radius * 2f)));
                    break;

                case Event.EventDataOneofCase.TargetRemoved:
                    if (!Guid.TryParse(received.TargetRemoved.TargetId, out var removedId) || removedId == Guid.Empty)
                    {
                        Debug.LogWarning("[GameMaster] 的の削除通知に有効な UUID がありません。");
                        break;
                    }
                    TargetRemoved?.Invoke(removedId);
                    break;

                default:
                    Debug.Log($"[GameMaster] unhandled event: {received.EventDataCase}");
                    break;
            }
        }

        public async Task<bool> AddTargetAsync(TargetCoordinate coordinate, double radius, CancellationToken token = default)
        {
            if (_client == null || !_running || token.IsCancellationRequested) return false;
            try
            {
                await _client.AddTargetAsync(new AddTargetRequest
                {
                    MachineId = _settings.machineId,
                    Compass = new ProtoTarget.Types.Compass
                    {
                        X = coordinate.X,
                        Y = coordinate.Y,
                        Radius = (float)radius,
                    },
                }, cancellationToken: token);
                return true;
            }
            catch (RpcException ex)
            {
                if (!token.IsCancellationRequested)
                    Debug.LogWarning($"[GameMaster] 的の登録に失敗しました: {ex.StatusCode} {ex.Status.Detail}");
                return false;
            }
        }

        public async Task<bool> RemoveTargetAsync(Guid targetId, CancellationToken token = default)
        {
            if (_client == null || !_running || token.IsCancellationRequested) return false;
            try
            {
                await _client.RemoveTargetAsync(new RemoveTargetRequest
                {
                    MachineId = _settings.machineId,
                    TargetId = targetId.ToString(),
                }, cancellationToken: token);
                return true;
            }
            catch (RpcException ex)
            {
                if (!token.IsCancellationRequested)
                    Debug.LogWarning($"[GameMaster] 的の削除に失敗しました: {ex.StatusCode} {ex.Status.Detail}");
                return false;
            }
        }

        public async Task<bool> HitTargetAsync(Guid targetId, CancellationToken token = default)
        {
            if (_client == null || !_running || token.IsCancellationRequested) return false;
            try
            {
                await _client.HitTargetAsync(new HitTargetRequest
                {
                    MachineId = _settings.machineId,
                    TargetId = targetId.ToString(),
                }, cancellationToken: token);
                return true;
            }
            catch (RpcException ex)
            {
                if (!token.IsCancellationRequested)
                    Debug.LogWarning($"[GameMaster] 命中の通知に失敗しました: {ex.StatusCode} {ex.Status.Detail}");
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
            _running = false;
        }
    }
}
