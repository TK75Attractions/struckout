using UnityEngine;
using StruckOut.Infrastructure.Network;

namespace StruckOut.Bootstrap
{
    public class NetworkBootstrap : MonoBehaviour
    {
        private TCPClientService _tcpClient;

        private async void Start()
        {
            _tcpClient = new TCPClientService();
            await _tcpClient.ConnectAsync("127.0.0.1", 5000);
        }
    }
}