using System;
using System.Linq;
using Struckout.Domain;
using System.Collections.Generic;
using Struckout.Domain.TargetDefinition;
using System.Threading;
using Cysharp.Threading.Tasks;
using Struckout.Application.Interface;
using Struckout.Domain.Ranking;
using Tk75Attractions.Struckout.GameMaster.V1;
using Microsoft.Extensions.Logging;
using Target = Struckout.Domain.TargetDefinition.Target;
using Stopwatch = System.Diagnostics.Stopwatch;

namespace Struckout.Infrastructure.Fake
{
    /// <summary>
    /// game_master のかわり。接続直後に一度だけ GameStarted を流し、
    /// 残り時間と累計得点を通知し、制限時間が尽きたら GameFinished を流す。
    /// </summary>
    public class FakeGameMasterClient : IGameMasterClient
    {
        private readonly ILogger<FakeGameMasterClient> _logger;

        /// <summary>game_master の GAME_DURATION (services/mod.rs) に合わせてある。</summary>
        private readonly TimeSpan _gameDuration;
        private readonly TimeSpan _screenDuration;
        private static readonly PlayerId FakePlayer = new(1);
        private uint _nextGameId = 20;
        private GameId _currentGameId;
        private readonly Dictionary<GameId, uint> _results = new()
        {
            [new GameId(5)] = 1500,
            [new GameId(12)] = 500,
            [new GameId(13)] = 800,
        };

        public FakeGameMasterClient(ILogger<FakeGameMasterClient> logger)
            : this(TimeSpan.FromSeconds(90), TimeSpan.FromSeconds(5), logger) { }

        internal FakeGameMasterClient(TimeSpan gameDuration, TimeSpan screenDuration, ILogger<FakeGameMasterClient> logger)
        {
            _logger = logger ?? throw new ArgumentNullException(nameof(logger));
            if (gameDuration < TimeSpan.Zero) throw new ArgumentOutOfRangeException(nameof(gameDuration));
            if (screenDuration < TimeSpan.Zero) throw new ArgumentOutOfRangeException(nameof(screenDuration));
            _gameDuration = gameDuration;
            _screenDuration = screenDuration;
        }

        /// <summary>
        /// 購読が始まる前に開始してしまった場合に備えて、開始したことを覚えておく。
        ///
        /// 本物は購読を始めたあとストリームから流れてくるが、ダミーは
        /// <see cref="ConnectAsync"/> の中で即座に発火するため、
        /// 後から購読した相手にも追いつかせる。
        /// </summary>
        public event Action<Difficulty, GameMode, PlayerId, TimeSpan> GameStarted
        {
            add
            {
                lock (_stateLock)
                {
                    _gameStarted += value;
                    if (_started) value?.Invoke(_startedDifficulty, GameMode.Single, FakePlayer, _gameDuration);
                }
            }
            remove
            {
                lock (_stateLock) _gameStarted -= value;
            }
        }

        private Action<Difficulty, GameMode, PlayerId, TimeSpan> _gameStarted;
        private bool _started;
        private Difficulty _startedDifficulty;

        public event Action<GameId> GameFinished;
        public event Action RankingScreenRequested;
        public event Action HomeScreenRequested;
        public event Action<TimeSpan> RemainingTimeChanged;
        public event Action<uint> ScoreChanged;
        public event Action<uint> ComboChanged;
        public event Action<Target> TargetAdded;
        public event Action<Guid, TargetOwnership> PocketWatchAdded;
        public event Action<Guid> TargetRemoved;

        // ダミーは自分から切る以外に切れないので、この経路は発火しない。
        public event Action ConnectionLost;

        private CancellationTokenSource _gameCancellation;
        private UniTask? _gameTask;
        private readonly object _stateLock = new();
        private uint _totalScore;
        private uint _combo;
        private static readonly (uint Combo, uint Bonus)[] ComboBonuses = { (3, 500), (5, 800) };
        private readonly Dictionary<Guid, double> _targetRadii = new();
        private bool _connected;
        private Guid? _pocketWatchId;

        public UniTask<bool> ConnectAsync()
        {
            lock (_stateLock)
            {
                if (_connected) return UniTask.FromResult(true);
                _connected = true;

                _logger.LogInformation("[Fake] game_master connected");

                RemoveAllTargets();
                _currentGameId = new GameId(++_nextGameId);
                _started = true;
                _totalScore = 0;
                _combo = 0;
                _startedDifficulty = Difficulty.Normal;
                _gameCancellation = new CancellationTokenSource();
                var elapsed = Stopwatch.StartNew();
                _gameStarted?.Invoke(_startedDifficulty, GameMode.Single, FakePlayer, _gameDuration);
                AddPocketWatch();
                RemainingTimeChanged?.Invoke(_gameDuration);
                ScoreChanged?.Invoke(_totalScore);

                _gameTask = RunGameAsync(elapsed, _gameCancellation.Token);

                return UniTask.FromResult(true);
            }
        }

        /// <summary>
        /// 本物と同じく、制限時間が尽きたら終了を通知する。
        /// 実装が終了を無視していないかを Fake モードでも確かめられるようにしてある。
        /// </summary>
        private async UniTask RunGameAsync(Stopwatch elapsed, CancellationToken token)
        {
            try
            {
                while (true)
                {
                    var remaining = _gameDuration - elapsed.Elapsed;
                    if (remaining > TimeSpan.Zero)
                        await UniTask.Delay(remaining < TimeSpan.FromSeconds(1) ? remaining : TimeSpan.FromSeconds(1), DelayType.Realtime, cancellationToken: token, cancelImmediately: true);

                    lock (_stateLock)
                    {
                        if (token.IsCancellationRequested) return;
                        remaining = _gameDuration - elapsed.Elapsed;
                        if (remaining <= TimeSpan.Zero)
                        {
                            _started = false;
                            _targetRadii.Clear();
                            _pocketWatchId = null;
                            RemainingTimeChanged?.Invoke(TimeSpan.Zero);
                            _logger.LogInformation("[Fake] game_master -> GameFinished");
                            _results[_currentGameId] = _totalScore;
                            GameFinished?.Invoke(_currentGameId);
                            break;
                        }

                        RemainingTimeChanged?.Invoke(remaining);
                    }
                }
                if (_screenDuration > TimeSpan.Zero)
                    await UniTask.Delay(_screenDuration, DelayType.Realtime, cancellationToken: token, cancelImmediately: true);
                lock (_stateLock)
                {
                    if (token.IsCancellationRequested) return;
                    RankingScreenRequested?.Invoke();
                }
                if (_screenDuration > TimeSpan.Zero)
                    await UniTask.Delay(_screenDuration, DelayType.Realtime, cancellationToken: token, cancelImmediately: true);
                lock (_stateLock)
                {
                    if (token.IsCancellationRequested) return;
                    HomeScreenRequested?.Invoke();
                }
            }
            catch (OperationCanceledException)
            {
            }
        }

        public UniTask<bool> AddTargetAsync(TargetCoordinate coordinate, double radius, CancellationToken token = default)
        {
            lock (_stateLock)
            {
                if (!_connected || !_started || token.IsCancellationRequested) return UniTask.FromResult(false);
                var id = Guid.NewGuid();
                // Match protobuf's float precision before the server calculates in f64.
                _targetRadii.Add(id, (float)radius);
                TargetAdded?.Invoke(new Target(id,
                    new TargetGeometry(coordinate, TargetType.Compass, (float)radius * 2f), TargetOwnership.Single));
                return UniTask.FromResult(true);
            }
        }

        public UniTask<bool> RemoveTargetAsync(Guid targetId, CancellationToken token = default)
        {
            lock (_stateLock)
            {
                if (!_connected || !_started || token.IsCancellationRequested || !_targetRadii.Remove(targetId))
                    return UniTask.FromResult(false);
                TargetRemoved?.Invoke(targetId);
                return UniTask.FromResult(true);
            }
        }

        private void RemoveAllTargets()
        {
            var ids = new List<Guid>(_targetRadii.Keys);
            if (_pocketWatchId.HasValue) ids.Add(_pocketWatchId.Value);
            _pocketWatchId = null;
            _targetRadii.Clear();
            foreach (var id in ids) TargetRemoved?.Invoke(id);
        }

        private void AddPocketWatch()
        {
            _pocketWatchId = Guid.NewGuid();
            PocketWatchAdded?.Invoke(_pocketWatchId.Value, TargetOwnership.Single);
        }

        public UniTask<bool> ResetComboAsync(CancellationToken token = default)
        {
            lock (_stateLock)
            {
                if (!_connected || !_started || token.IsCancellationRequested) return UniTask.FromResult(false);
                if (_combo != 0)
                {
                    _combo = 0;
                    ComboChanged?.Invoke(0);
                }
                return UniTask.FromResult(true);
            }
        }

        public UniTask<bool> HitTargetAsync(Guid targetId, CancellationToken token = default)
        {
            lock (_stateLock)
            {
                if (!_connected || !_started || token.IsCancellationRequested) return UniTask.FromResult(false);
                var watch = _pocketWatchId == targetId;
                var points = 0.0;
                if (!watch)
                {
                    if (!_targetRadii.TryGetValue(targetId, out var radius) ||
                        double.IsNaN(radius) || double.IsInfinity(radius) || radius <= 0)
                        return UniTask.FromResult(false);
                    points = Math.Truncate(100000.0 / radius);
                }
                if (_combo == uint.MaxValue) return UniTask.FromResult(false);
                var combo = _combo + 1;
                var bonus = ComboBonuses.FirstOrDefault(item => item.Combo == combo).Bonus;
                if (points > uint.MaxValue || points + bonus + _totalScore > uint.MaxValue) return UniTask.FromResult(false);
                _totalScore += (uint)points + bonus;
                _combo = combo;
                ScoreChanged?.Invoke(_totalScore);
                ComboChanged?.Invoke(_combo);
                if (watch)
                {
                    _pocketWatchId = null;
                    TargetRemoved?.Invoke(targetId);
                    AddPocketWatch();
                }
                return UniTask.FromResult(true);
            }
        }

        public UniTask<uint?> GetGameResultAsync(GameId gameId, CancellationToken token = default)
        {
            lock (_stateLock)
                return UniTask.FromResult<uint?>(_connected && !token.IsCancellationRequested && _results.TryGetValue(gameId, out var score) ? score : null);
        }

        public UniTask<IReadOnlyList<RankingEntry>> GetRankingAsync(uint maxItemCount, GameMode mode, Difficulty difficulty, CancellationToken token = default)
        {
            lock (_stateLock)
            {
                if (!_connected || token.IsCancellationRequested) return UniTask.FromResult<IReadOnlyList<RankingEntry>>(null);
                if (mode != GameMode.Single || difficulty != _startedDifficulty) return UniTask.FromResult<IReadOnlyList<RankingEntry>>(Array.Empty<RankingEntry>());
                var entries = Enumerable.Range(1, 10).Select(i => new RankingEntry(new GameId((uint)i),
                    (uint)(2000 - i * 100), i == 5 ? FakePlayer : new PlayerId((uint)i + 1),
                    i == 5 ? "サンプル" : "プレイヤー" + i)).ToList();
                foreach (var game in _results.Where(game => game.Key.Value > 10))
                    entries.Add(new RankingEntry(game.Key, game.Value, FakePlayer, "サンプル"));
                return UniTask.FromResult<IReadOnlyList<RankingEntry>>(entries.OrderByDescending(entry => entry.Score)
                    .ThenBy(entry => entry.GameId.Value).Take((int)Math.Min(maxItemCount, int.MaxValue)).ToArray());
            }
        }

        public UniTask<IReadOnlyList<RankingEntry>> GetGamesByPlayerAsync(PlayerId playerId, GameMode mode, Difficulty difficulty, CancellationToken token = default)
        {
            lock (_stateLock)
            {
                if (!_connected || token.IsCancellationRequested) return UniTask.FromResult<IReadOnlyList<RankingEntry>>(null);
                return UniTask.FromResult<IReadOnlyList<RankingEntry>>(playerId.Equals(FakePlayer) && mode == GameMode.Single && difficulty == _startedDifficulty
                    ? _results.Select(game => new RankingEntry(game.Key, game.Value, FakePlayer, "サンプル")).ToArray() : Array.Empty<RankingEntry>());
            }
        }

        public async UniTask DisconnectAsync()
        {
            UniTask? gameTask;
            CancellationTokenSource cancellation;
            lock (_stateLock)
            {
                if (!_connected) return;
                _connected = false;
                _started = false;
                cancellation = _gameCancellation;
                gameTask = _gameTask;
                _gameCancellation = null;
                _gameTask = null;
                cancellation?.Cancel();
            }

            if (gameTask.HasValue) await gameTask.Value;
            cancellation?.Dispose();

            _logger.LogInformation("[Fake] game_master disconnected");
        }
    }
}
