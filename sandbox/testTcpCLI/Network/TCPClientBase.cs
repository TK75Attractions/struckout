using System.Net.Sockets;
using System.Threading.Tasks;
using System;
using System.Buffers.Binary;
using Google.Protobuf;
using System.Threading;
using System.IO;
using Struckout.Application;

namespace Struckout.Infrastructure
{
    public class TCPClientBase<T> : IServerService<T>
    {
        bool _isConnected = false;

        int _port;
        private TcpListener listener;
        private bool isRegister = false;

        public bool GetOpen() => _isConnected;
        public int GetPort() => _port;

        public void RegisterPort(int port)
        {
            if (isRegister)
            {
                Console.WriteLine("Already Registered");
                return;
            }

            _port = port;
            listener = new(System.Net.IPAddress.Loopback, port);

            isRegister = true;
        }

        public async Task Open()
        {
            if (_isConnected || !isRegister) return;
            listener.Start();
            _isConnected = true;
        }

        public async Task Close()
        {
            if (!_isConnected || !isRegister) return;
            listener.Stop();
            _isConnected = false;
        }
    }
}