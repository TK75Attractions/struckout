using Struckout.Application;
using System;
using Tk75Attractions.Struckout.V1;
using System.Threading.Tasks;
using System.Threading;
using UnityEngine;

namespace Struckout.Infrastructure
{
    public class FakeClientService : IClientService
    {
        public void RegisterPort(string host,int port){}
        public event Action<ProjectorPacket> OnCollisionReceived;
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
            System.Random random = new ();
            while (_isConnected || !token.IsCancellationRequested)
            {
                
                float x = (float)random.NextDouble()*4;
                float y = (float)random.NextDouble()*4;
                ProjectorPacket ProjectorPacket = new ProjectorPacket
                {
                    Point = new CollisionPoint
                    {
                        X = x,
                        Y = y  
                    }
                };
                Debug.Log(x.ToString() + " " + y.ToString());

                OnCollisionReceived?.Invoke(ProjectorPacket);
                await Task.Delay(1000, token);
            }
        }
    }
}