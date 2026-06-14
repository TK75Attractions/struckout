using UnityEngine;
using Struckout.Infrastructure.Network;
using Struckout.Infrastructure;
using Struckout.Debug;


namespace Struckout.Bootstrap
{
    public class NetworkBootstrap
    {
        private TCPClientService _tcpClient;
        private PacketRouter packetRouter = new();

        private async void Start()
        {
            packetRouter.AddStringMessageAction(OnReceiveMessage);
            
            _tcpClient = new TCPClientService();
            
            _tcpClient.AddAction(packetRouter.RoutePacket);

            await _tcpClient.ConnectAsync("127.0.0.1", 5000);

        }

        private void OnReceiveMessage(StringMessage message)
        {
            UnityEngine.Debug.Log($"Received message: {message.Message}");
            // Handle the received string message
        }

        private void OnDestroy()
        {
            _tcpClient?.DisconnectAsync();
        }
    }
}