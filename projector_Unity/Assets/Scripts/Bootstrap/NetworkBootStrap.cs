using UnityEngine;
using Struckout.Infrastructure.Network;
using Struckout.Application;
using Struckout.Debug;
using Cysharp.Threading.Tasks;


namespace Struckout.Bootstrap
{
    public class NetworkBootstrap : IAsyncDestroy
    {
        private TCPClientService _tcpClient;
        private IPacketRouter packetRouter;

        internal async UniTask Initialize(RuntimeContext context)
        {   
            packetRouter = context.packetRouter;
            _tcpClient = context.TCPClient;

            packetRouter.AddStringMessageAction(OnReceiveMessage);
            
            
            _tcpClient.AddAction(packetRouter.RoutePacket);

            bool isSuccessfullyConnect = await _tcpClient.ConnectAsync("127.0.0.1", 5000);
            if(!isSuccessfullyConnect) throw new System.Exception("Failed to connect successfully");
        }

        private void OnReceiveMessage(StringMessage message)
        {
            UnityEngine.Debug.Log($"Received message: {message.Message}");
            // Handle the received string message
        }

        public async UniTask OnDestroy()
        {
            if (_tcpClient == null) return;
            _tcpClient.RemoveAction(packetRouter.RoutePacket);
            await _tcpClient.DisconnectAsync();
        }
    }
}