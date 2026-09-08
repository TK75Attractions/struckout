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
        public event Action ConnectionLost;
        private Task _receiveTask;
        private readonly IMessageParser<T> _parser;
        private bool isRegister = false;
        private readonly SemaphoreSlim _slim = new(1, 1);
        private readonly SemaphoreSlim _sendSlim = new(1, 1);

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
                    // 再試行で毎回 TcpClient を捨てないように、失敗した分はここで解放する。
                    _tcpClient.Dispose();
                    _tcpClient = null;
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
                // Failed (通信中に切れた) からも後片付けできるようにする。
                if (_state != ConnectionState.Connected
                    && _state != ConnectionState.Connecting
                    && _state != ConnectionState.Failed) return;
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

        public async Task<bool> ConnectRetryAsync(int maxAttempts)
        {
            for(int attempt = 0; attempt < maxAttempts; attempt++)
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

        public async Task<bool> SendAsync(IMessage packet)
        {
            if (packet == null) throw new ArgumentNullException(nameof(packet));

            var stream = _networkStream;
            if (_state != ConnectionState.Connected || stream == null)
            {
                Debug.LogWarning("Tried to send a packet while not connected.");
                return false;
            }

            byte[] payload = packet.ToByteArray();
            byte[] frame = new byte[4 + payload.Length];
            BinaryPrimitives.WriteUInt32LittleEndian(frame.AsSpan(0, 4), (uint)payload.Length);
            payload.CopyTo(frame, 4);

            // 送信の途中に別の送信が割り込むとフレームが壊れるので直列化する。
            await _sendSlim.WaitAsync();
            try
            {
                await stream.WriteAsync(frame, 0, frame.Length);
                await stream.FlushAsync();
                return true;
            }
            catch (Exception ex) when (ex is IOException || ex is ObjectDisposedException || ex is SocketException)
            {
                Debug.LogWarning($"Failed to send a packet: {ex.Message}");
                return false;
            }
            finally
            {
                _sendSlim.Release();
            }
        }

        private async Task ReceiveDataAsync(CancellationToken token)
        {
            // 自分から切ったのか、相手都合で切れたのかを区別する。
            // 後者だけを ConnectionLost として通知したい。
            bool lostUnexpectedly = false;

            while (_state == ConnectionState.Connected && !token.IsCancellationRequested)
            {
                byte[] data;
                if (_tcpClient == null || _networkStream == null)
                {
                    lostUnexpectedly = true;
                    break;
                }

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
                    Debug.LogWarning($"Connection lost while receiving: {ex.Message}");
                    lostUnexpectedly = true;
                    break;
                }
                catch (IOException ex)
                {
                    Debug.LogWarning($"Connection lost while receiving: {ex.Message}");
                    lostUnexpectedly = true;
                    break;
                }
                catch (ObjectDisposedException ex)
                {
                    Debug.LogWarning($"Connection lost while receiving: {ex.Message}");
                    lostUnexpectedly = true;
                    break;
                }
                catch (Exception ex)
                {
                    Debug.LogException(ex);
                    lostUnexpectedly = true;
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

            if (lostUnexpectedly) HandleConnectionLost();
        }

        /// <summary>
        /// 相手都合で切れたときの後始末。
        /// これをやらないと状態が Connected のまま残り、切れているのに接続中に見えてしまう。
        /// SendAsync も死んだストリームに書きに行くことになる。
        /// </summary>
        private void HandleConnectionLost()
        {
            if (_state != ConnectionState.Connected) return;

            Transit(ConnectionState.Failed);

            _networkStream?.Dispose();
            _networkStream = null;
            _tcpClient?.Dispose();
            _tcpClient = null;

            try
            {
                ConnectionLost?.Invoke();
            }
            catch (Exception ex)
            {
                Debug.LogException(ex);
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