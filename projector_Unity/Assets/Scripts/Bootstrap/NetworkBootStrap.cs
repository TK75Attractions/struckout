using UnityEngine;
using Struckout.Infrastructure.Network;
using Struckout.Application;
using Struckout.Debug;
using Cysharp.Threading.Tasks;


namespace Struckout.Bootstrap
{
    public class NetworkBootstrap
    {
        private TCPClientService _tcpClient;
        private IPacketRouter packetRouter;

        internal async UniTask Initialize(RuntimeContext context)
        {
            context.AddDestroyEvent(OnDestroy);
            
            packetRouter = context.packetRouter;
            _tcpClient = context.TCPClient;

            packetRouter.AddStringMessageAction(OnReceiveMessage);
            
            
            _tcpClient.AddAction(packetRouter.RoutePacket);

            await _tcpClient.ConnectAsync("127.0.0.1", 5000);
        }

        private void OnReceiveMessage(StringMessage message)
        {
            UnityEngine.Debug.Log($"Received message: {message.Message}");
            // Handle the received string message
        }

        private async void OnDestroy()
        {
            await _tcpClient?.DisconnectAsync();
        }
    }
}