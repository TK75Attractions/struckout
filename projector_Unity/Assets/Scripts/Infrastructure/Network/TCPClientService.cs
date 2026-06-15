using System.Net.Sockets;
using System.Text;
using System.Threading.Tasks;
using System;
using Struckout.Dto.V1;
using System.Buffers.Binary;
using Google.Protobuf;

namespace Struckout.Infrastructure.Network
{
    public class TCPClientService
    {
        bool isConnected = false;
        private TcpClient _tcpClient;
        private NetworkStream  _networkStream;

        private Action<NetworkPacket> _onCollisionReceived;

        public void AddAction(Action<NetworkPacket> action)
        {
            _onCollisionReceived += action;
        }

        public async Task ConnectAsync(string host, int port)
        {
            _tcpClient = new();
            try
            {
                
                await _tcpClient.ConnectAsync(host, port);
                _networkStream = _tcpClient.GetStream();
                isConnected = true;
                Console.WriteLine("Connected to TCP server.");
            }
            catch (Exception ex)
            {
                Console.WriteLine($"Error connecting to TCP server: {ex.Message}");
            }

            _ = ReceiveDataAsync();
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

        #region ReadMethod

        private async Task ReceiveDataAsync()
        {
            while (isConnected)
            {
                byte[] data;
                try
                {
                    data = await ReadByteAsync();
                }
                catch (InvalidProtocolBufferException ex)
                {
                    UnityEngine.Debug.Log(ex);
                    continue;
                }
                catch (OperationCanceledException ex)
                {
                    UnityEngine.Debug.Log(ex);
                    throw;
                }
                catch (Exception ex)
                {
                    UnityEngine.Debug.Log(ex);
                    continue;
                }

                var packet = NetworkPacket.Parser.ParseFrom(data);

                var handlerList = _onCollisionReceived.GetInvocationList();

                foreach (Action<NetworkPacket> handle in handlerList)
                {
                    try
                    {
                        handle(packet);
                    }
                    catch (Exception ex)
                    {
                        UnityEngine.Debug.Log(ex);
                    }
                }
            }
        }

        private async Task<byte[]> ReadByteAsync()
        {
            byte[] lengthBuffer = new byte[4];
            await ReadExactAsync(lengthBuffer, 4);
            uint length = BinaryPrimitives.ReadUInt32LittleEndian(lengthBuffer);
            byte[] dataBuffer = new byte[length];
            await ReadExactAsync(dataBuffer, (int)length);
            return dataBuffer;
        }

        private async Task ReadExactAsync(byte[] buffer, int length)
        {
            int totalRead = length;
            int offset = 0;

            while (offset < totalRead)
            {
                int received = await _networkStream.ReadAsync(buffer, offset, totalRead - offset);
                if (received == 0)
                {
                    throw new Exception("Connection closed by the server.");
                }
                offset += received;
            }
        } 
        #endregion
    }
}