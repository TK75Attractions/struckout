using Struckout.Application;
using System;
using Struckout.Dto.V1;
using System.Threading.Tasks;
using System.Threading;

namespace Struckout.Infrastructure
{
    public class FakeClientService : IClientService
    {
        public void RegisterPort(string host,int port){}
        public event Action<NetworkPacket> OnCollisionReceived;
        private bool _isConnected;
        CancellationTokenSource _receiveCancellationToken;
        Task task;
        public async Task<bool> ConnectAsync()
        {
            _isConnected = true;
            _receiveCancellationToken = new CancellationTokenSource();
            task = ReceieveCollision(_receiveCancellationToken.Token);
            return true;
        }
        public async Task DisconnectAsync()
        {
            _isConnected = false;
            _receiveCancellationToken.Cancel();
            await task;
            return;
        }

        private async Task ReceieveCollision(CancellationToken token)
        {
            while (_isConnected || !token.IsCancellationRequested)
            {
                float x = UnityEngine.Random.Range(0,4f);
                float y = UnityEngine.Random.Range(0,4f);
                NetworkPacket networkPacket = new NetworkPacket
                {
                    Point = new CollisionPoint
                    {
                        X = x,
                        Y = y  
                    }
                };
                UnityEngine.Debug.Log(x.ToString() + " " + y.ToString());

                OnCollisionReceived?.Invoke(networkPacket);
                await Task.Delay(1000, token);
            }
        }
    }
}