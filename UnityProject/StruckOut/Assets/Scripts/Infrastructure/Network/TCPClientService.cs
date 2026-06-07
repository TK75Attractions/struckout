using System.Net.Sockets;
using System.Text;
using System.Threading.Tasks;
using System;

namespace StruckOut.Infrastructure.Network
{
    public class TCPClientService
    {
        bool isConnected = false;
        private TcpClient _tcpClient;
        private NetworkStream  _networkStream;

        public async Task ConnectAsync(string host, int port)
        {
            _tcpClient = new();
            try
            {
                isConnected = true;
                await _tcpClient.ConnectAsync(host, port);
                _networkStream = _tcpClient.GetStream();
                Console.WriteLine("Connected to TCP server.");
            }
            catch (Exception ex)
            {
                Console.WriteLine($"Error connecting to TCP server: {ex.Message}");
            }

            await Task.CompletedTask;
        }

        public async Task DisconnectAsync()
        {
            if (!isConnected || _tcpClient == null) return;

            try
            {
                _networkStream?.Dispose();
                _tcpClient?.Dispose();
                isConnected = false;
                Console.WriteLine("Disconnected from TCP server.");
            }
            catch (Exception ex)
            {
                Console.WriteLine($"Error closing connection to TCP server: {ex.Message}");
            }

            await Task.CompletedTask;
        }
    }
}