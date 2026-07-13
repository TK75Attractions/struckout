using System.Net.Sockets;
using System.Threading.Tasks;
using System;
using System.Buffers.Binary;
using Google.Protobuf;
using System.Threading;
using System.IO;
using UnityEngine;
using Struckout.Application;
using Struckout.Domain;

namespace Struckout.Infrastructure
{
    public class TCPClientBase<T> : IClientService<T>
    {
        ConnectionState _state = ConnectionState.Disconnected;
        private string _host;
        private int _port;
        private TcpClient _tcpClient;
        private NetworkStream  _networkStream;
        private CancellationTokenSource _receiveCancellationToken;
        public event Action<T> OnReceived;
        private Task _receiveTask;
        private readonly IMessageParser<T> _parser;
        private bool isRegister = false;
        private readonly SemaphoreSlim _slim = new(1, 1);

        private ConnectionState Transit(ConnectionState to) =>_state = ConnectionStateMachine.Transition(_state, to);

        public TCPClientBase(IMessageParser<T> parser)
        {
            _parser = parser;
        }

        public void RegisterPort(string host, int port)
        {
            _host = host;
            _port = port;
            isRegister = true;
        }

        public async Task<bool> ConnectAsync()
        {
            await _slim.WaitAsync();

            try
            {
                Transit(ConnectionState.Connecting);
                if (!isRegister)
                {
                    Transit(ConnectionState.Failed);
                    throw new Exception("Haven't been register port");
                }
                
                _tcpClient = new();
                try
                {
                    await _tcpClient.ConnectAsync(_host, _port);
                    _networkStream = _tcpClient.GetStream();
                    Transit(ConnectionState.Connected);
                    Debug.Log("Connected to TCP server.");
                }
                catch (Exception ex)
                {
                    Debug.Log($"Error connecting to TCP server: {ex.Message}");
                    Transit(ConnectionState.Failed);
                    return false;
                }

                if (_state == ConnectionState.Connected)
                {
                    _receiveCancellationToken = new CancellationTokenSource();
                    _receiveTask = ReceiveDataAsync(_receiveCancellationToken.Token);
                    return true;
                }

                return false;
            }
            finally
            {
                _slim.Release();
            }
        }

        public async Task DisconnectAsync()
        {
            await _slim.WaitAsync();
            
            try
            {
                if (_state != ConnectionState.Connected && _state != ConnectionState.Connecting) return;
                if(_tcpClient == null) Debug.Log("Failed To Disconnect");
                
                Transit(ConnectionState.Disconnecting);
                
                _receiveCancellationToken?.Cancel();
                try
                {
                    if(_receiveTask != null) await _receiveTask;
                }
                catch (Exception ex)
                {
                    Debug.Log($"Error closing connection to TCP server: {ex.Message}");
                }
                finally
                {
                    _networkStream?.Dispose();
                    _tcpClient?.Dispose();

                    Transit(ConnectionState.Disconnected);
                }

                Debug.Log("Done");

                await Task.CompletedTask;
            }
            finally
            {
                _slim.Release();
            }
        }

        public async Task<bool> ConnectRetryAsync(int maxattempts)
        {
            for(int attempt = 0; attempt < maxattempts; attempt++)
            {
                try
                {
                    if(await ConnectAsync())
                    {
                        return true;
                    }
                }
                catch(Exception ex)
                {
                    Debug.LogWarning($"Connect attempt failed because of {ex}");
                }
            
                await Task.Delay(TimeSpan.FromSeconds(Math.Pow(2, attempt)));
            }
            return false;
        }

        private async Task ReceiveDataAsync(CancellationToken token)
        {
            while (_state == ConnectionState.Connected && !token.IsCancellationRequested)
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

                T packet;
                try
                {
                    packet = _parser.MessageParse(data);
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

                var handlerList = OnReceived?.GetInvocationList();
                if (handlerList == null) continue;

                foreach (Action<T> handle in handlerList)
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
    }
}