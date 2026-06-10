using UnityEngine;
using StruckOut.Infrastructure.Network;
using StruckOut.Application;
using StruckOut.Debug;


namespace StruckOut.Bootstrap
{
    public class NetworkBootstrap : MonoBehaviour
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

        private void OnReceiveMessage(stringMessage message)
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