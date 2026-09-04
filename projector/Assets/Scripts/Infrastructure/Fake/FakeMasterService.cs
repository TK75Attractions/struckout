using Struckout.Application;
using Google.Protobuf;
using System;
using Tk75Attractions.Struckout.V1;
using System.Threading.Tasks;
using System.Threading;
using UnityEngine;

namespace Struckout.Infrastructure
{
    /// <summary>
    /// game_master (タッチパネル) のかわり。
    /// 接続直後に一度だけ StartGame を流し、あとは黙っている。
    /// </summary>
    public class FakeMasterService : IClientService<MasterProjectorPacket>
    {
        private const int StartDelayMilliseconds = 500;

        public event Action<MasterProjectorPacket> OnReceived;

        private bool _isConnected;
        private CancellationTokenSource _receiveCancellationToken;
        private Task _task;

        public void RegisterPort(string host, int port) { }

        public Task<bool> ConnectAsync()
        {
            if (_isConnected) return Task.FromResult(true);

            _isConnected = true;
            _receiveCancellationToken = new CancellationTokenSource();
            _task = StartGameAsync(_receiveCancellationToken.Token);

            Debug.Log("[Fake] master connected");
            return Task.FromResult(true);
        }

        public Task<bool> ConnectRetryAsync(int maxAttempts) => ConnectAsync();

        public Task<bool> SendAsync(IMessage packet)
        {
            // 対向がいないので送りっぱなしにできない。届いた内容だけ残す。
            Debug.Log($"[Fake] master <- {packet.Descriptor.Name} {packet}");
            return Task.FromResult(_isConnected);
        }

        public async Task DisconnectAsync()
        {
            if (!_isConnected) return;

            Debug.Log("[Fake] master disconnected");
            _isConnected = false;
            _receiveCancellationToken.Cancel();

            try
            {
                await _task;
            }
            catch (OperationCanceledException)
            {
                // 想定内
            }

            _receiveCancellationToken.Dispose();
            _receiveCancellationToken = null;
            _task = null;
        }

        private async Task StartGameAsync(CancellationToken token)
        {
            try
            {
                await Task.Delay(StartDelayMilliseconds, token);
            }
            catch (OperationCanceledException)
            {
                return;
            }

            if (!_isConnected || token.IsCancellationRequested) return;

            MasterProjectorPacket packet = new()
            {
                StartGame = new StartGame
                {
                    Difficulty = Difficulty.Normal
                }
            };

            Debug.Log("[Fake] StartGame(NORMAL)");
            OnReceived?.Invoke(packet);
        }
    }
}
