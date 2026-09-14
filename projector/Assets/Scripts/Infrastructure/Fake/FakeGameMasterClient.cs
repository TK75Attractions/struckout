using System;
using System.Threading;
using System.Threading.Tasks;
using Struckout.Application;
using Tk75Attractions.Struckout.V1;
using UnityEngine;

namespace Struckout.Infrastructure
{
    /// <summary>
    /// game_master のかわり。接続直後に一度だけ GameStarted を流し、
    /// 制限時間ぶん待ってから GameFinished を流す。
    /// 得点はログに出すだけで、どこにも送らない。
    /// </summary>
    public class FakeGameMasterClient : IGameMasterClient
    {
        /// <summary>game_master の GAME_DURATION (service.rs) に合わせてある。</summary>
        private static readonly TimeSpan GameDuration = TimeSpan.FromSeconds(150);

        /// <summary>
        /// 購読が始まる前に開始してしまった場合に備えて、開始したことを覚えておく。
        ///
        /// 本物は購読を始めたあとストリームから流れてくるが、ダミーは
        /// <see cref="ConnectAsync"/> の中で即座に発火する。呼ぶ側は
        /// ConnectAsync を await したあとで購読するので、素直に発火すると
        /// 誰も居ないところへ投げることになり、Fake モードでゲームが
        /// 永遠に始まらない。後から購読した相手にも追いつかせる。
        /// </summary>
        public event Action<Difficulty> GameStarted
        {
            add
            {
                _gameStarted += value;
                if (_started) value(_startedDifficulty);
            }
            remove => _gameStarted -= value;
        }

        private Action<Difficulty> _gameStarted;
        private bool _started;
        private Difficulty _startedDifficulty;

        public event Action GameFinished;

        // ダミーは自分から切る以外に切れないので、この経路は発火しない。
        public event Action ConnectionLost;

        private CancellationTokenSource _gameCancellation;
        private bool _connected;

        public Task<bool> ConnectAsync()
        {
            if (_connected) return Task.FromResult(true);
            _connected = true;

            Debug.Log("[Fake] game_master connected");

            _started = true;
            _startedDifficulty = Difficulty.Normal;
            _gameStarted?.Invoke(_startedDifficulty);

            _gameCancellation = new CancellationTokenSource();
            _ = FinishAfterDurationAsync(_gameCancellation.Token);

            return Task.FromResult(true);
        }

        /// <summary>
        /// 本物と同じく、制限時間が尽きたら終了を通知する。
        /// 実装が終了を無視していないかを Fake モードでも確かめられるようにしてある。
        /// </summary>
        private async Task FinishAfterDurationAsync(CancellationToken token)
        {
            try
            {
                await Task.Delay(GameDuration, token);
            }
            catch (OperationCanceledException)
            {
                return;
            }

            Debug.Log("[Fake] game_master -> GameFinished");
            GameFinished?.Invoke();
        }

        public Task<bool> AddScoreAsync(int scoreToAdd)
        {
            Debug.Log($"[Fake] game_master <- AddScore(+{scoreToAdd})");
            return Task.FromResult(_connected);
        }

        public Task DisconnectAsync()
        {
            if (!_connected) return Task.CompletedTask;
            _connected = false;
            _started = false;

            _gameCancellation?.Cancel();
            _gameCancellation?.Dispose();
            _gameCancellation = null;

            Debug.Log("[Fake] game_master disconnected");
            return Task.CompletedTask;
        }
    }
}
