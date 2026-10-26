using System;
using System.Linq;
using Struckout.Domain;
using System.Collections.Generic;
using Struckout.Domain.TargetDefinition;
using System.Threading;
using System.Threading.Tasks;
using Struckout.Application.Interface;
using Tk75Attractions.Struckout.GameMaster.V1;
using UnityEngine;
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

        public FakeGameMasterClient() : this(TimeSpan.FromSeconds(90), TimeSpan.FromSeconds(5)) { }

        internal FakeGameMasterClient(TimeSpan gameDuration, TimeSpan screenDuration)
        {
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
        public event Action<Difficulty, PlayerId> GameStarted
        {
            add
            {
                lock (_stateLock)
                {
                    _gameStarted += value;
                    if (_started) value?.Invoke(_startedDifficulty, FakePlayer);
                }
            }
            remove
            {
                lock (_stateLock) _gameStarted -= value;
            }
        }

        private Action<Difficulty, PlayerId> _gameStarted;
        private bool _started;
        private Difficulty _startedDifficulty;

        public event Action<GameId> GameFinished;
        public event Action RankingScreenRequested;
        public event Action HomeScreenRequested;
        public event Action<TimeSpan> RemainingTimeChanged;
        public event Action<uint> ScoreChanged;
        public event Action<Target> TargetAdded;
        public event Action<Guid> TargetRemoved;

        // ダミーは自分から切る以外に切れないので、この経路は発火しない。
        public event Action ConnectionLost;

        private CancellationTokenSource _gameCancellation;
        private Task _gameTask;
        private readonly object _stateLock = new();
        private uint _totalScore;
        private readonly Dictionary<Guid, double> _targetRadii = new();
        private bool _connected;

        public Task<bool> ConnectAsync()
        {
            lock (_stateLock)
            {
                if (_connected) return Task.FromResult(true);
                _connected = true;

                Debug.Log("[Fake] game_master connected");

                RemoveAllTargets();
                _currentGameId = new GameId(++_nextGameId);
                _started = true;
                _totalScore = 0;
                _startedDifficulty = Difficulty.Normal;
                _gameCancellation = new CancellationTokenSource();
                var elapsed = Stopwatch.StartNew();
                _gameStarted?.Invoke(_startedDifficulty, FakePlayer);
                RemainingTimeChanged?.Invoke(_gameDuration);
                ScoreChanged?.Invoke(_totalScore);

                _gameTask = RunGameAsync(elapsed, _gameCancellation.Token);

                return Task.FromResult(true);
            }
        }

        /// <summary>
        /// 本物と同じく、制限時間が尽きたら終了を通知する。
        /// 実装が終了を無視していないかを Fake モードでも確かめられるようにしてある。
        /// </summary>
        private async Task RunGameAsync(Stopwatch elapsed, CancellationToken token)
        {
            try
            {
                while (true)
                {
                    var remaining = _gameDuration - elapsed.Elapsed;
                    if (remaining > TimeSpan.Zero)
                        await Task.Delay(remaining < TimeSpan.FromSeconds(1) ? remaining : TimeSpan.FromSeconds(1), token);

                    lock (_stateLock)
                    {
                        if (token.IsCancellationRequested) return;
                        remaining = _gameDuration - elapsed.Elapsed;
                        if (remaining <= TimeSpan.Zero)
                        {
                            _started = false;
                            RemoveAllTargets();
                            RemainingTimeChanged?.Invoke(TimeSpan.Zero);
                            Debug.Log("[Fake] game_master -> GameFinished");
                            _results[_currentGameId] = _totalScore;
                            GameFinished?.Invoke(_currentGameId);
                            break;
                        }

                        RemainingTimeChanged?.Invoke(remaining);
                    }
                }
                await Task.Delay(_screenDuration, token);
                lock (_stateLock)
                {
                    if (token.IsCancellationRequested) return;
                    RankingScreenRequested?.Invoke();
                }
                await Task.Delay(_screenDuration, token);
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

        public Task<bool> AddTargetAsync(TargetCoordinate coordinate, double radius, CancellationToken token = default)
        {
            lock (_stateLock)
            {
                if (!_connected || !_started || token.IsCancellationRequested) return Task.FromResult(false);
                var id = Guid.NewGuid();
                // Match protobuf's float precision before the server calculates in f64.
                _targetRadii.Add(id, (float)radius);
                TargetAdded?.Invoke(new Target(id,
                    new TargetGeometry(coordinate, TargetType.Circle, (float)radius * 2f), TargetOwnership.Single));
                return Task.FromResult(true);
            }
        }

        public Task<bool> RemoveTargetAsync(Guid targetId, CancellationToken token = default)
        {
            lock (_stateLock)
            {
                if (!_connected || !_started || token.IsCancellationRequested || !_targetRadii.Remove(targetId))
                    return Task.FromResult(false);
                TargetRemoved?.Invoke(targetId);
                return Task.FromResult(true);
            }
        }

        private void RemoveAllTargets()
        {
            var ids = new List<Guid>(_targetRadii.Keys);
            _targetRadii.Clear();
            foreach (var id in ids) TargetRemoved?.Invoke(id);
        }

        public Task<bool> HitTargetAsync(Guid targetId, CancellationToken token = default)
        {
            lock (_stateLock)
            {
                if (!_connected || !_started || token.IsCancellationRequested || !_targetRadii.TryGetValue(targetId, out var radius) ||
                    double.IsNaN(radius) || double.IsInfinity(radius) || radius <= 0)
                    return Task.FromResult(false);
                var points = Math.Truncate(100000.0 / radius);
                if (points > uint.MaxValue || points + _totalScore > uint.MaxValue) return Task.FromResult(false);
                _totalScore += (uint)points;
                ScoreChanged?.Invoke(_totalScore);
                return Task.FromResult(true);
            }
        }

        public Task<uint?> GetGameResultAsync(GameId gameId, CancellationToken token = default)
        {
            lock (_stateLock)
                return Task.FromResult<uint?>(_connected && !token.IsCancellationRequested && _results.TryGetValue(gameId, out var score) ? score : null);
        }

        public Task<IReadOnlyList<RankingEntry>> GetRankingAsync(uint maxItemCount, CancellationToken token = default)
        {
            lock (_stateLock)
            {
                if (!_connected || token.IsCancellationRequested) return Task.FromResult<IReadOnlyList<RankingEntry>>(null);
                var entries = Enumerable.Range(1, 10).Select(i => new RankingEntry(new GameId((uint)i),
                    (uint)(2000 - i * 100), i == 5 ? FakePlayer : new PlayerId((uint)i + 1),
                    i == 5 ? "サンプル" : "プレイヤー" + i)).ToList();
                foreach (var game in _results.Where(game => game.Key.Value > 10))
                    entries.Add(new RankingEntry(game.Key, game.Value, FakePlayer, "サンプル"));
                return Task.FromResult<IReadOnlyList<RankingEntry>>(entries.OrderByDescending(entry => entry.Score)
                    .ThenBy(entry => entry.GameId.Value).Take((int)Math.Min(maxItemCount, int.MaxValue)).ToArray());
            }
        }

        public Task<IReadOnlyList<PlayerGame>> GetGamesByPlayerAsync(PlayerId playerId, CancellationToken token = default)
        {
            lock (_stateLock)
            {
                if (!_connected || token.IsCancellationRequested) return Task.FromResult<IReadOnlyList<PlayerGame>>(null);
                return Task.FromResult<IReadOnlyList<PlayerGame>>(playerId.Equals(FakePlayer)
                    ? _results.Select(game => new PlayerGame(game.Key, game.Value)).ToArray() : Array.Empty<PlayerGame>());
            }
        }

        public async Task DisconnectAsync()
        {
            Task gameTask;
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

            if (gameTask != null) await gameTask;
            cancellation?.Dispose();

            Debug.Log("[Fake] game_master disconnected");
        }
    }
}
