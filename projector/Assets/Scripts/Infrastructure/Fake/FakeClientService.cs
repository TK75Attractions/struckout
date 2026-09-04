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
    /// ball_watcher のかわりに、ランダムな衝突点を流し続けるダミー。
    /// TCP の対向を用意せずに描画だけを確認したいときに使う。
    /// </summary>
    public class FakeClientService : IClientService<ProjectorPacket>
    {
        // CollisionPoint は物理座標 (m)。ball_watcher は三角測量の結果をそのまま送る
        // (ball_watcher/src/collision_output/network.rs: x = coll.x, y = coll.z)。
        // 盤面の実寸が決まったら直すこと。
        private const float FieldMinX = -1f;
        private const float FieldMaxX = 1f;
        private const float FieldMinY = 0f;
        private const float FieldMaxY = 2f;

        private const int IntervalMilliseconds = 1000;

        public event Action<ProjectorPacket> OnReceived;

        private bool _isConnected;
        private CancellationTokenSource _receiveCancellationToken;
        private Task _task;

        public void RegisterPort(string host, int port) { }

        public Task<bool> ConnectAsync()
        {
            if (_isConnected) return Task.FromResult(true);

            _isConnected = true;
            _receiveCancellationToken = new CancellationTokenSource();
            _task = ReceiveCollisionAsync(_receiveCancellationToken.Token);

            Debug.Log("[Fake] tracker connected");
            return Task.FromResult(true);
        }

        public Task<bool> ConnectRetryAsync(int maxAttempts) => ConnectAsync();

        public Task<bool> SendAsync(IMessage packet)
        {
            // 対向がいないので送りっぱなしにできない。届いた内容だけ残す。
            Debug.Log($"[Fake] tracker <- {packet.Descriptor.Name} {packet}");
            return Task.FromResult(_isConnected);
        }

        public async Task DisconnectAsync()
        {
            if (!_isConnected) return;

            Debug.Log("[Fake] tracker disconnected");
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

        private async Task ReceiveCollisionAsync(CancellationToken token)
        {
            System.Random random = new();

            while (_isConnected && !token.IsCancellationRequested)
            {
                float x = FieldMinX + (float)random.NextDouble() * (FieldMaxX - FieldMinX);
                float y = FieldMinY + (float)random.NextDouble() * (FieldMaxY - FieldMinY);

                ProjectorPacket packet = new()
                {
                    Point = new CollisionPoint
                    {
                        X = x,
                        Y = y
                    }
                };

                Debug.Log($"[Fake] collision at ({x:F3}, {y:F3}) m");
                OnReceived?.Invoke(packet);

                try
                {
                    await Task.Delay(IntervalMilliseconds, token);
                }
                catch (OperationCanceledException)
                {
                    return;
                }
            }
        }
    }
}
