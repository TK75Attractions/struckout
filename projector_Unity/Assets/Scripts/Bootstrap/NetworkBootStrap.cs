using UnityEngine;
using Struckout.Infrastructure.Network;
using Struckout.Application;
using Struckout.Debug;
using Cysharp.Threading.Tasks;


namespace Struckout.Bootstrap
{
    public class NetworkBootstrap : IAsyncDestroy
    {
        private IClientService _Client;
        private IPacketRouter packetRouter;

        internal async UniTask Initialize(RuntimeContext context)
        {   
            packetRouter = context.PacketRouter;
            _Client = context.Client;

            packetRouter._onStringMessageReceived += OnReceiveMessage;
            
            
            _Client._onCollisionReceived += packetRouter.RoutePacket;

            bool isSuccessfullyConnect = await _Client.ConnectAsync("127.0.0.1", 5000);
            if(!isSuccessfullyConnect) throw new System.Exception("Failed to connect successfully");
        }

        private void OnReceiveMessage(StringMessage message)
        {
            UnityEngine.Debug.Log($"Received message: {message.Message}");
            // Handle the received string message
        }

        public async UniTask OnDestroy()
        {
            if (_Client == null) return;
            _Client._onCollisionReceived -= packetRouter.RoutePacket;
            packetRouter._onStringMessageReceived -= OnReceiveMessage;
            await _Client.DisconnectAsync();
        }
    }
}