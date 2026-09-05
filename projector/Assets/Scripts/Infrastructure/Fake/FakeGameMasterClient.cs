using System;
using System.Threading.Tasks;
using Struckout.Application;
using Tk75Attractions.Struckout.V1;
using UnityEngine;

// UnityEngine.Event と名前が衝突するので、proto 側を明示する。
using Event = Tk75Attractions.Struckout.V1.Event;

namespace Struckout.Infrastructure
{
    /// <summary>
    /// game_master のかわり。接続直後に一度だけ GameStarted を流す。
    /// 得点はログに出すだけで、どこにも送らない。
    /// </summary>
    public class FakeGameMasterClient : IGameMasterClient
    {
        private const int FakeGameId = 1;

        public event Action<Event.Types.GameStarted> GameStarted;

        // ダミーは自分から切る以外に切れないので、この経路は発火しない。
        public event Action ConnectionLost;

        private bool _connected;

        public Task<bool> ConnectAsync()
        {
            if (_connected) return Task.FromResult(true);
            _connected = true;

            Debug.Log("[Fake] game_master connected");

            GameStarted?.Invoke(new Event.Types.GameStarted
            {
                MachineId = 0,
                Difficulty = Difficulty.Normal,
                GameId = FakeGameId,
            });

            return Task.FromResult(true);
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
            Debug.Log("[Fake] game_master disconnected");
            return Task.CompletedTask;
        }
    }
}
