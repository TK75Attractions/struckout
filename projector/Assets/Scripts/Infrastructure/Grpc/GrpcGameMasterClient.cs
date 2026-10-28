using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading;
using Cysharp.Threading.Tasks;
using Cysharp.Net.Http;
using Grpc.Core;
using Grpc.Net.Client;
using JetBrains.Annotations;
using Struckout.Application.Interface;
using Struckout.Domain;
using Struckout.Domain.Ranking;
using Tk75Attractions.Struckout.GameMaster.V1;
using Struckout.Domain.TargetDefinition;
using Struckout.Domain.Targets;
using Microsoft.Extensions.Logging;
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
        private readonly ILogger<GrpcGameMasterClient> _logger;

        private readonly NetworkSettings _settings;

        private YetAnotherHttpHandler _handler;
        private GrpcChannel _channel;
        private GameMasterSessionService.GameMasterSessionServiceClient _client;

        private CancellationTokenSource _listenCancellation;
        private UniTask? _listenTask;

        /// <summary>machine_id に対応するゲームの開始・終了通知から管理する。</summary>
        private volatile bool _running;

        /// <summary>game_master と切れたとき、購読をやり直す間隔。</summary>
        private static readonly TimeSpan ReconnectInterval = TimeSpan.FromSeconds(2);

        public event Action<Difficulty, GameMode, PlayerId, TimeSpan> GameStarted;
        public event Action<GameId> GameFinished;
        public event Action RankingScreenRequested;
        public event Action HomeScreenRequested;
        public event Action<TimeSpan> RemainingTimeChanged;
        public event Action<uint> ScoreChanged;
        public event Action ConnectionLost;
        public event Action<DomainTarget> TargetAdded;
        public event Action<Guid, TargetOwnership> PocketWatchAdded;
        public event Action<Guid> TargetRemoved;

        [UsedImplicitly(ImplicitUseKindFlags.InstantiatedNoFixedConstructorSignature)]
        public GrpcGameMasterClient(NetworkSettings settings, ILogger<GrpcGameMasterClient> logger)
        {
            _logger = logger ?? throw new ArgumentNullException(nameof(logger));
            _settings = settings ?? throw new ArgumentNullException(nameof(settings));
        }

        public UniTask<bool> ConnectAsync()
        {
            if (_channel != null) return UniTask.FromResult(true);

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
                _logger.LogError(ex, "[GameMaster] failed to open a channel to {Address}: {ErrorMessage}", address, ex.Message);
                Cleanup();
                return UniTask.FromResult(false);
            }

            _listenCancellation = new CancellationTokenSource();
            _listenTask = ListenEventsAsync(_listenCancellation.Token);

            _logger.LogInformation("[GameMaster] listening for events on {Address} as machine {MachineId}", address, _settings.machineId);
            return UniTask.FromResult(true);
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
        private async UniTask ListenEventsAsync(CancellationToken token)
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
                        _logger.LogInformation("[GameMaster] reconnected; listening for events again.");
                        lost = false;
                    }

                    while (await call.ResponseStream.MoveNext(token))
                    {
                        var received = call.ResponseStream.Current?.Event;
                        if (received == null)
                        {
                            _logger.LogWarning("[GameMaster] received a response without an event; ignored.");
                            continue;
                        }

                        HandleEvent(received);
                    }

                    _logger.LogWarning("[GameMaster] the event stream ended.");
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
                    if (!lost) _logger.LogWarning(ex, "[GameMaster] the event stream failed: {StatusCode} {StatusDetail}", ex.Status.StatusCode, ex.Status.Detail);
                }
                catch (Exception ex)
                {
                    if (!lost) _logger.LogError(ex, "処理中に例外が発生しました。");
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
                        _logger.LogError(ex, "処理中に例外が発生しました。");
                    }
                }

                try
                {
                    await UniTask.Delay(ReconnectInterval, DelayType.Realtime, cancellationToken: token, cancelImmediately: true);
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
                    var playerId = LocalPlayer(received.GameStarted);
                    if (!playerId.HasValue)
                    {
                        _logger.LogWarning("[GameMaster] 開始通知に自分の筐体のプレイヤー情報がありません。");
                        break;
                    }
                    _running = true;
                    GameStarted?.Invoke(received.GameStarted.Difficulty,
                        received.GameStarted.PlayerInfo.Single != null ? GameMode.Single : GameMode.Multi, playerId.Value,
                        TimeSpan.FromSeconds(received.GameStarted.GameDurSecs));
                    break;

                case Event.EventDataOneofCase.GameFinished:
                    _running = false;
                    GameFinished?.Invoke(new GameId(received.GameFinished.GameId));
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
                        _logger.LogWarning("[GameMaster] received an invalid remaining duration; ignored.");
                        break;
                    }
                    RemainingTimeChanged?.Invoke(remainingTime);
                    break;

                case Event.EventDataOneofCase.GameScoreChanged:
                    if (!_running) break;
                    ScoreChanged?.Invoke(received.GameScoreChanged.TotalScore);
                    break;

                case Event.EventDataOneofCase.NavigateToRankingScreenRequested:
                    if (received.NavigateToRankingScreenRequested.MachineId == _settings.machineId)
                        RankingScreenRequested?.Invoke();
                    break;

                case Event.EventDataOneofCase.NavigateToHomeScreenRequsted:
                    if (received.NavigateToHomeScreenRequsted.MachineId == _settings.machineId)
                        HomeScreenRequested?.Invoke();
                    break;

                case Event.EventDataOneofCase.TargetAdded:
                    var added = received.TargetAdded;
                    var data = added.Target;
                    if (data == null || !Guid.TryParse(data.TargetId, out var targetId) || targetId == Guid.Empty)
                    {
                        _logger.LogWarning("[GameMaster] 的の追加通知に有効な UUID または的データがありません。");
                        break;
                    }

                    if (!data.IsOwnedBy(_settings.machineId)) break;
                    if (data.TargetDataCase == ProtoTarget.TargetDataOneofCase.PocketWatch)
                    {
                        PocketWatchAdded?.Invoke(targetId,
                            data.OwnerCase == ProtoTarget.OwnerOneofCase.All ? TargetOwnership.All : TargetOwnership.Single);
                        break;
                    }
                    var compass = data.Compass;
                    if (compass == null || float.IsNaN(compass.X) || float.IsInfinity(compass.X) ||
                        float.IsNaN(compass.Y) || float.IsInfinity(compass.Y) ||
                        float.IsNaN(compass.Radius) || compass.Radius <= 0 || float.IsInfinity(compass.Radius * 2f))
                    {
                        _logger.LogWarning("[GameMaster] 的の追加通知に有効な円の形状がありません。");
                        break;
                    }
                    TargetAdded?.Invoke(new DomainTarget(targetId,
                        new TargetGeometry(new TargetCoordinate(compass.X, compass.Y), TargetType.Compass, compass.Radius * 2f),
                        data.OwnerCase == ProtoTarget.OwnerOneofCase.All ? TargetOwnership.All : TargetOwnership.Single));
                    break;

                case Event.EventDataOneofCase.TargetRemoved:
                    if (!Guid.TryParse(received.TargetRemoved.TargetId, out var removedId) || removedId == Guid.Empty)
                    {
                        _logger.LogWarning("[GameMaster] 的の削除通知に有効な UUID がありません。");
                        break;
                    }
                    TargetRemoved?.Invoke(removedId);
                    break;

                default:
                    _logger.LogInformation("[GameMaster] unhandled event: {EventKind}", received.EventDataCase);
                    break;
            }
        }

        private PlayerId? LocalPlayer(Event.Types.GameStarted started)
        {
            var players = PlayerItems(started.PlayerInfo);
            var local = players.Where(player => player != null && player.MachineId == _settings.machineId).ToArray();
            return local.Length == 1 ? new PlayerId(local[0].PlayerId) : null;
        }

        private static PlayerInfo.Types.PlayerItem[] PlayerItems(PlayerInfo info) => info?.PlayerInfoDataCase switch
        {
            PlayerInfo.PlayerInfoDataOneofCase.Single => new[] { info.Single.Player },
            PlayerInfo.PlayerInfoDataOneofCase.Multi => new[] { info.Multi.Player1, info.Multi.Player2 },
            _ => Array.Empty<PlayerInfo.Types.PlayerItem>(),
        };

        private static RankingEntry RankingGame(uint gameId, uint score, PlayerInfo info) =>
            new(new GameId(gameId), score, PlayerItems(info).Where(player => player != null)
                .Select(player => new RankingParticipant(player.MachineId, new PlayerId(player.PlayerId), player.PlayerName)).ToArray());

        public async UniTask<uint?> GetGameResultAsync(GameId gameId, CancellationToken token = default)
        {
            if (_client == null || token.IsCancellationRequested) return null;
            try
            {
                var response = await _client.GetGameResultAsync(new GetGameResultRequest { GameId = gameId.Value }, cancellationToken: token);
                return response.TotalScore;
            }
            catch (RpcException error)
            {
                if (!token.IsCancellationRequested) _logger.LogWarning(error, "[GameMaster] 結果の取得に失敗しました: {StatusCode} {StatusDetail}", error.StatusCode, error.Status.Detail);
                return null;
            }
        }

        public async UniTask<IReadOnlyList<RankingEntry>> GetRankingAsync(uint maxItemCount, GameMode mode, Difficulty difficulty, CancellationToken token = default)
        {
            if (_client == null || token.IsCancellationRequested) return null;
            try
            {
                var response = await _client.GetRankingAsync(new GetRankingRequest { MaxItemNum = maxItemCount, Mode = mode, Difficulty = difficulty }, cancellationToken: token);
                return response.TopN.Select(item => RankingGame(item.GameId, item.TotalScore, item.PlayerInfo)).ToArray();
            }
            catch (RpcException error)
            {
                if (!token.IsCancellationRequested) _logger.LogWarning(error, "[GameMaster] ランキングの取得に失敗しました: {StatusCode} {StatusDetail}", error.StatusCode, error.Status.Detail);
                return null;
            }
        }

        public async UniTask<IReadOnlyList<RankingEntry>> GetGamesByPlayerAsync(PlayerId playerId, GameMode mode, Difficulty difficulty, CancellationToken token = default)
        {
            if (_client == null || token.IsCancellationRequested) return null;
            try
            {
                var response = await _client.GetGamesByPlayerAsync(new GetGamesByPlayerRequest { PlayerId = playerId.Value, Mode = mode, Difficulty = difficulty }, cancellationToken: token);
                return response.Items.Select(item => RankingGame(item.GameId, item.TotalScore, item.PlayerInfo)).ToArray();
            }
            catch (RpcException error)
            {
                if (!token.IsCancellationRequested) _logger.LogWarning(error, "[GameMaster] プレイヤーの履歴を取得できませんでした: {StatusCode} {StatusDetail}", error.StatusCode, error.Status.Detail);
                return null;
            }
        }

        public async UniTask<bool> AddTargetAsync(TargetCoordinate coordinate, double radius, CancellationToken token = default)
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
                    _logger.LogWarning(ex, "[GameMaster] 的の登録に失敗しました: {StatusCode} {StatusDetail}", ex.StatusCode, ex.Status.Detail);
                return false;
            }
        }

        public async UniTask<bool> RemoveTargetAsync(Guid targetId, CancellationToken token = default)
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
                    _logger.LogWarning(ex, "[GameMaster] 的の削除に失敗しました: {StatusCode} {StatusDetail}", ex.StatusCode, ex.Status.Detail);
                return false;
            }
        }

        public async UniTask<bool> HitTargetAsync(Guid targetId, CancellationToken token = default)
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
                    _logger.LogWarning(ex, "[GameMaster] 命中の通知に失敗しました: {StatusCode} {StatusDetail}", ex.StatusCode, ex.Status.Detail);
                return false;
            }
        }

        public async UniTask DisconnectAsync()
        {
            if (_channel == null) return;

            _listenCancellation?.Cancel();

            if (_listenTask != null)
            {
                try
                {
                    await _listenTask.Value;
                }
                catch (Exception ex)
                {
                    _logger.LogWarning(ex, "[GameMaster] the event stream ended with {ExceptionType}", ex.GetType().Name);
                }
            }

            Cleanup();
            _logger.LogInformation("[GameMaster] disconnected");
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
