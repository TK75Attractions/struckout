using System.Net.Sockets;
using System.Threading.Tasks;
using System;
using Tk75Attractions.Struckout.V1;
using System.Buffers.Binary;
using Google.Protobuf;
using System.Threading;
using System.IO;
using Struckout.Application;
using UnityEngine;

namespace Struckout.Infrastructure.Network
{
    public class TCPClientService : IClientService
    {
        bool _isConnected = false;
        private string _host;
        private int _port;
        private TcpClient _tcpClient;
        private NetworkStream  _networkStream;
        private CancellationTokenSource _receiveCancellationToken;
        public event Action<ProjectorPacket> OnCollisionReceived;
        private Task _receiveTask;
        private bool isRegister = false;

        public void RegisterPort(string host, int port)
        {
            _host = host;
            _port = port;
            isRegister = true;
        }

        public async Task<bool> ConnectAsync()
        {
            if (!isRegister) throw new Exception("Haven't been register port");
            _tcpClient = new();
            try
            {
                await _tcpClient.ConnectAsync(_host, _port);
                _networkStream = _tcpClient.GetStream();
                _isConnected = true;
                Debug.Log("Connected to TCP server.");
            }
            catch (Exception ex)
            {
                Debug.Log($"Error connecting to TCP server: {ex.Message}");
                return false;
            }

            if (_isConnected)
            {
                _receiveCancellationToken = new CancellationTokenSource();
                _receiveTask = ReceiveDataAsync(_receiveCancellationToken.Token);
                return true;
            }
            return false;
        }

        public async Task DisconnectAsync()
        {
            if (!_isConnected || _tcpClient == null) return;

            try
            {
                _receiveCancellationToken.Cancel();
                await _receiveTask;
                
                Debug.Log("Disconnected from TCP server.");
            }
            catch (Exception ex)
            {
                Debug.Log($"Error closing connection to TCP server: {ex.Message}");
            }
            finally
            {
                try
                {
                    _networkStream?.Dispose();
                    _tcpClient?.Dispose();

                    _isConnected = false;
                }
                catch (Exception ex)
                {
                    Debug.Log($"Error closing connection to TCP server: {ex.Message}");
                }
            }

            await Task.CompletedTask;
        }

        #region ReadMethod

        private async Task ReceiveDataAsync(CancellationToken token)
        {
            while (_isConnected && !token.IsCancellationRequested)
            {
                byte[] data;
                if (_tcpClient == null || _networkStream == null) break;

                try
                {
                    data = await ReadByteAsync(token);
                }
                catch (OperationCanceledException) when (token.IsCancellationRequested)
                {
                    break;
                }
                catch (EndOfStreamException ex)
                {
                    Debug.Log(ex);
                    break;
                }
                catch (IOException ex)
                {
                    Debug.Log(ex);
                    break;
                }
                catch (ObjectDisposedException ex)
                {
                    Debug.Log(ex);
                    break;
                }
                catch (Exception ex)
                {
                    Debug.Log(ex);
                    break;
                }

                ProjectorPacket packet;
                try
                {
                    packet = ProjectorPacket.Parser.ParseFrom(data);
                }
                catch (InvalidProtocolBufferException ex)
                {
                    Debug.Log(ex);
                    continue;
                }
                catch
                {
                    Debug.Log("Failed to Parse");
                    continue;
                }

                var handlerList = OnCollisionReceived?.GetInvocationList();
                if (handlerList == null) continue;

                foreach (Action<ProjectorPacket> handle in handlerList)
                {
                    try
                    {
                        handle(packet);
                    }
                    catch (Exception ex)
                    {
                        Debug.LogException(ex);
                    }
                }
            }
        }

        private async Task<byte[]> ReadByteAsync(CancellationToken token)
        {
            byte[] lengthBuffer = new byte[4];
            await ReadExactAsync(lengthBuffer, 4, token);
            uint length = BinaryPrimitives.ReadUInt32LittleEndian(lengthBuffer);
            byte[] dataBuffer = new byte[length];
            await ReadExactAsync(dataBuffer, (int)length, token);
            return dataBuffer;
        }

        private async Task ReadExactAsync(byte[] buffer, int length, CancellationToken token)
        {
            int totalRead = length;
            int offset = 0;

            while (offset < totalRead)
            {
                int received = await _networkStream.ReadAsync(buffer, offset, totalRead - offset, token);
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