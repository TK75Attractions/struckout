using System;
using System.Buffers.Binary;
using System.IO;
using System.Net.Sockets;
using System.Threading;
using Cysharp.Threading.Tasks;
using Google.Protobuf;
using JetBrains.Annotations;
using Struckout.Application.Interface;
using Struckout.Domain;
using Microsoft.Extensions.Logging;

namespace Struckout.Infrastructure.TCP
{
    public class TcpClientBase<T> : IClientService<T>
    {
        private readonly ILogger<TcpClientBase<T>> _logger;

        ConnectionState _state = ConnectionState.Disconnected;
        private string _host;
        private int _port;
        private TcpClient _tcpClient;
        private NetworkStream _networkStream;
        private CancellationTokenSource _receiveCancellationToken;
        public event Action<T> OnReceived;
        public event Action ConnectionLost;
        private UniTask? _receiveTask;
        private readonly IMessageParser<T> _parser;
        private bool _isRegister;
        private readonly SemaphoreSlim _slim = new(1, 1);
        private readonly SemaphoreSlim _sendSlim = new(1, 1);
        private const double MaxRetryDelaySeconds = 5;

        private void Transit(ConnectionState to)
        {
            _state = ConnectionStateMachine.Transition(_state, to);
        }

        [UsedImplicitly(ImplicitUseKindFlags.InstantiatedNoFixedConstructorSignature)]
        public TcpClientBase(IMessageParser<T> parser, ILogger<TcpClientBase<T>> logger)
        {
            _logger = logger ?? throw new ArgumentNullException(nameof(logger));
            _parser = parser;
        }

        public void RegisterPort(string host, int port)
        {
            _host = host;
            _port = port;
            _isRegister = true;
        }

        public async UniTask<bool> ConnectAsync()
        {
            await _slim.WaitAsync();

            try
            {
                Transit(ConnectionState.Connecting);
                if (!_isRegister)
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
                    _logger.LogInformation("Connected to TCP server.");
                }
                catch (Exception ex)
                {
                    _logger.LogInformation(ex, "Error connecting to TCP server: {ErrorMessage}", ex.Message);
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

        public async UniTask DisconnectAsync()
        {
            await _slim.WaitAsync();

            try
            {
                // Failed (通信中に切れた) からも後片付けできるようにする。
                if (_state != ConnectionState.Connected
                    && _state != ConnectionState.Connecting
                    && _state != ConnectionState.Failed) return;
                if (_tcpClient == null) _logger.LogInformation("Failed To Disconnect");

                Transit(ConnectionState.Disconnecting);

                _receiveCancellationToken?.Cancel();
                try
                {
                    if (_receiveTask.HasValue) await _receiveTask.Value;
                }
                catch (Exception ex)
                {
                    _logger.LogInformation(ex, "Error closing connection to TCP server: {ErrorMessage}", ex.Message);
                }
                finally
                {
                    _networkStream?.Dispose();
                    _tcpClient?.Dispose();

                    Transit(ConnectionState.Disconnected);
                }

                _logger.LogInformation("Done");

            }
            finally
            {
                _slim.Release();
            }
        }

        public async UniTask<bool> ConnectRetryAsync(uint maxAttempts)
        {
            for (var attempt = 0; attempt < maxAttempts; attempt++)
            {
                try
                {
                    if (await ConnectAsync())
                    {
                        return true;
                    }
                }
                catch (Exception ex)
                {
                    _logger.LogWarning(ex, "Connect attempt failed because of {ErrorMessage}", ex.Message);
                }

                // 指数バックオフ。ただし相手 (ball_tracker) が後から起動したときにすぐつながるよう、間隔は 5 秒までにする
                await UniTask.Delay(TimeSpan.FromSeconds(Math.Min(Math.Pow(2, attempt), MaxRetryDelaySeconds)), DelayType.Realtime);
            }
            return false;
        }

        public async UniTask<bool> SendAsync(IMessage packet)
        {
            if (packet == null) throw new ArgumentNullException(nameof(packet));

            var stream = _networkStream;
            if (_state != ConnectionState.Connected || stream == null)
            {
                _logger.LogWarning("Tried to send a packet while not connected.");
                return false;
            }

            var payload = packet.ToByteArray();
            var frame = new byte[4 + payload.Length];
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
                _logger.LogWarning(ex, "Failed to send a packet: {ErrorMessage}", ex.Message);
                return false;
            }
            finally
            {
                _sendSlim.Release();
            }
        }

        private async UniTask ReceiveDataAsync(CancellationToken token)
        {
            // 自分から切ったのか、相手都合で切れたのかを区別する。
            // 後者だけを ConnectionLost として通知したい。
            var lostUnexpectedly = false;

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
                    _logger.LogWarning(ex, "Connection lost while receiving: {ErrorMessage}", ex.Message);
                    lostUnexpectedly = true;
                    break;
                }
                catch (IOException ex)
                {
                    _logger.LogWarning(ex, "Connection lost while receiving: {ErrorMessage}", ex.Message);
                    lostUnexpectedly = true;
                    break;
                }
                catch (ObjectDisposedException ex)
                {
                    _logger.LogWarning(ex, "Connection lost while receiving: {ErrorMessage}", ex.Message);
                    lostUnexpectedly = true;
                    break;
                }
                catch (Exception ex)
                {
                    _logger.LogError(ex, "処理中に例外が発生しました。");
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
                    _logger.LogInformation(ex, "パケットを解析できませんでした。");
                    continue;
                }
                catch
                {
                    _logger.LogInformation("Failed to Parse");
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
                        _logger.LogError(ex, "処理中に例外が発生しました。");
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
                _logger.LogError(ex, "処理中に例外が発生しました。");
            }
        }

        private async UniTask<byte[]> ReadByteAsync(CancellationToken token)
        {
            var lengthBuffer = new byte[4];
            await ReadExactAsync(lengthBuffer, 4, token);
            var length = BinaryPrimitives.ReadUInt32LittleEndian(lengthBuffer);
            var dataBuffer = new byte[length];
            await ReadExactAsync(dataBuffer, (int)length, token);
            return dataBuffer;
        }

        private async UniTask ReadExactAsync(byte[] buffer, int length, CancellationToken token)
        {
            var totalRead = length;
            var offset = 0;

            while (offset < totalRead)
            {
                var received = await _networkStream.ReadAsync(buffer, offset, totalRead - offset, token);
                if (received == 0)
                {
                    throw new Exception("Connection closed by the server.");
                }
                offset += received;
            }
        }
    }
}
