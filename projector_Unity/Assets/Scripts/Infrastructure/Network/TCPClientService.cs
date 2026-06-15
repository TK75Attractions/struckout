using System.Net.Sockets;
using System.Text;
using System.Threading.Tasks;
using System;
using Struckout.Dto.V1;
using System.Buffers.Binary;
using Google.Protobuf;
using System.Threading;
using System.IO;

namespace Struckout.Infrastructure.Network
{
    public class TCPClientService
    {
        bool _isConnected = false;
        private TcpClient _tcpClient;
        private NetworkStream  _networkStream;
        private CancellationTokenSource _receiveCancellationToken;
        private Action<NetworkPacket> _onCollisionReceived;
        private Task _receiveTask;

        public void AddAction(Action<NetworkPacket> action)
        {
            _onCollisionReceived += action;
        }

        public void RemoveAction(Action<NetworkPacket> action)
        {
            _onCollisionReceived -= action;
        }

        public async Task<bool> ConnectAsync(string host, int port)
        {
            _tcpClient = new();
            try
            {
                await _tcpClient.ConnectAsync(host, port);
                _networkStream = _tcpClient.GetStream();
                _isConnected = true;
                UnityEngine.Debug.Log("Connected to TCP server.");
            }
            catch (Exception ex)
            {
                UnityEngine.Debug.Log($"Error connecting to TCP server: {ex.Message}");
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
                
                UnityEngine.Debug.Log("Disconnected from TCP server.");
            }
            catch (Exception ex)
            {
                UnityEngine.Debug.Log($"Error closing connection to TCP server: {ex.Message}");
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
                    UnityEngine.Debug.Log($"Error closing connection to TCP server: {ex.Message}");
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
                    UnityEngine.Debug.Log(ex);
                    break;
                }
                catch (IOException ex)
                {
                    UnityEngine.Debug.Log(ex);
                    break;
                }
                catch (ObjectDisposedException ex)
                {
                    UnityEngine.Debug.Log(ex);
                    break;
                }
                catch (Exception ex)
                {
                    UnityEngine.Debug.Log(ex);
                    continue;
                }

                NetworkPacket packet;
                try
                {
                    packet = NetworkPacket.Parser.ParseFrom(data);
                }
                catch (InvalidProtocolBufferException ex)
                {
                    UnityEngine.Debug.Log(ex);
                    continue;
                }
                catch
                {
                    UnityEngine.Debug.Log("Failed to Parse");
                    continue;
                }

                var handlerList = _onCollisionReceived?.GetInvocationList();
                if (handlerList == null) continue;

                foreach (Action<NetworkPacket> handle in handlerList)
                {
                    try
                    {
                        if (handle == null) continue;

                        handle(packet);
                    }
                    catch (Exception ex)
                    {
                        UnityEngine.Debug.Log(ex);
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