using System;
using System.Collections.Generic;
using Struckout.Domain.TargetDefinition;
using System.Threading;
using System.Threading.Tasks;
using Struckout.Application.Interface;
using Tk75Attractions.Struckout.GameMaster.V1;
using UnityEngine;
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
        private static readonly TimeSpan GameDuration = TimeSpan.FromSeconds(90);

        /// <summary>
        /// 購読が始まる前に開始してしまった場合に備えて、開始したことを覚えておく。
        ///
        /// 本物は購読を始めたあとストリームから流れてくるが、ダミーは
        /// <see cref="ConnectAsync"/> の中で即座に発火するため、
        /// 後から購読した相手にも追いつかせる。
        /// </summary>
        public event Action<Difficulty> GameStarted
        {
            add
            {
                lock (_stateLock)
                {
                    _gameStarted += value;
                    if (_started) value?.Invoke(_startedDifficulty);
                }
            }
            remove
            {
                lock (_stateLock) _gameStarted -= value;
            }
        }

        private Action<Difficulty> _gameStarted;
        private bool _started;
        private Difficulty _startedDifficulty;

        public event Action GameFinished;
        public event Action<TimeSpan> RemainingTimeChanged;
        public event Action<uint> ScoreChanged;

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

                _started = true;
                _totalScore = 0;
                _targetRadii.Clear();
                _startedDifficulty = Difficulty.Normal;
                _gameCancellation = new CancellationTokenSource();
                var elapsed = Stopwatch.StartNew();
                _gameStarted?.Invoke(_startedDifficulty);
                RemainingTimeChanged?.Invoke(GameDuration);
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
                    var remaining = GameDuration - elapsed.Elapsed;
                    if (remaining > TimeSpan.Zero)
                        await Task.Delay(remaining < TimeSpan.FromSeconds(1) ? remaining : TimeSpan.FromSeconds(1), token);

                    lock (_stateLock)
                    {
                        if (token.IsCancellationRequested) return;
                        remaining = GameDuration - elapsed.Elapsed;
                        if (remaining <= TimeSpan.Zero)
                        {
                            _started = false;
                            RemainingTimeChanged?.Invoke(TimeSpan.Zero);
                            Debug.Log("[Fake] game_master -> GameFinished");
                            GameFinished?.Invoke();
                            return;
                        }

                        RemainingTimeChanged?.Invoke(remaining);
                    }
                }
            }
            catch (OperationCanceledException)
            {
            }
        }

        public Task<Guid?> AddTargetAsync(TargetCoordinate coordinate, double radius, CancellationToken token = default)
        {
            lock (_stateLock)
            {
                if (!_connected || !_started || token.IsCancellationRequested) return Task.FromResult<Guid?>(null);
                var id = Guid.NewGuid();
                // Match protobuf's float precision before the server calculates in f64.
                _targetRadii.Add(id, (float)radius);
                return Task.FromResult<Guid?>(id);
            }
        }

        public Task<bool> RemoveTargetAsync(Guid targetId, CancellationToken token = default)
        {
            lock (_stateLock)
                return Task.FromResult(_connected && _started && !token.IsCancellationRequested && _targetRadii.Remove(targetId));
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

        public async Task DisconnectAsync()
        {
            Task gameTask;
            CancellationTokenSource cancellation;
            lock (_stateLock)
            {
                if (!_connected) return;
                _connected = false;
                _targetRadii.Clear();
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
