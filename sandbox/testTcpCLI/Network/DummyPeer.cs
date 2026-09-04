using System.Buffers.Binary;
using System.Net;
using System.Net.Sockets;
using Google.Protobuf;

namespace Struckout.TestCli;

/// <summary>
/// projector から見た対向サーバ (ball_tracker / game_master) のダミー。
///
/// 本物と同じく listen 側になり、projector が client として接続してくる。
/// フレーミングは LE u32 の長さ + protobuf 本体で、
/// api/rust/src/lib.rs の write_packet / read_packet と同じ形式。
///
/// 出力は一切自分で行わず、すべてイベントで通知する。
/// CLI はコンソールへ、GUI はウィンドウへ、と出力先を差し替えられるようにするため。
///
/// 注意: <see cref="Log"/> / <see cref="Connected"/> / <see cref="Disconnected"/> /
/// <see cref="FrameReceived"/> は accept・受信のバックグラウンドスレッドから発火する。
/// UI スレッドが必要な購読側は自分でマーシャリングすること (WinForms なら Control.BeginInvoke)。
/// </summary>
public sealed class DummyPeer : IAsyncDisposable
{
    /// <summary>受信フレームの上限。壊れた長さヘッダで巨大な確保をしないための保険。</summary>
    private const int MaxFrameBytes = 1 << 20;

    private readonly SemaphoreSlim _sendLock = new(1, 1);

    private TcpListener? _listener;
    private NetworkStream? _stream;
    private CancellationTokenSource? _cts;
    private Task? _acceptTask;

    /// <param name="name">ログに添える名前。</param>
    public DummyPeer(string name)
    {
        Name = name;
    }

    public string Name { get; }

    public int Port { get; private set; }

    public bool IsListening => _listener is not null;

    public bool IsConnected => _stream is not null;

    /// <summary>人間向けの経過報告。</summary>
    public event Action<string>? Log;

    /// <summary>projector が接続してきた。</summary>
    public event Action<EndPoint?>? Connected;

    /// <summary>projector との接続が切れた。</summary>
    public event Action? Disconnected;

    /// <summary>1 フレーム受信した。デコードは購読側の責務。</summary>
    public event Action<byte[]>? FrameReceived;

    public void Listen(int port)
    {
        if (_listener is not null)
        {
            Log?.Invoke($"already listening on port {Port}");
            return;
        }

        // 本物の ball_tracker / game_master と同じく 0.0.0.0 で待つ。
        // docs/machine_separation.md のとおり projector が別マシンにいても届くように。
        var listener = new TcpListener(IPAddress.Any, port);
        listener.Start();

        _listener = listener;
        Port = port;
        _cts = new CancellationTokenSource();
        _acceptTask = AcceptLoopAsync(_cts.Token);

        Log?.Invoke($"listening on 0.0.0.0:{port}");
    }

    public async Task StopAsync()
    {
        if (_listener is null) return;

        Log?.Invoke("stopping");

        _cts?.Cancel();
        _listener.Stop();

        if (_acceptTask is not null)
        {
            try
            {
                await _acceptTask;
            }
            catch (Exception ex)
            {
                Log?.Invoke($"accept loop ended with {ex.GetType().Name}: {ex.Message}");
            }
        }

        _cts?.Dispose();
        _cts = null;
        _acceptTask = null;
        _listener = null;
        _stream = null;
        Port = 0;
    }

    /// <summary>projector に 1 パケット送る。接続していなければ false。</summary>
    public async Task<bool> SendAsync(IMessage message)
    {
        var stream = _stream;
        if (stream is null)
        {
            Log?.Invoke("not connected; nothing was sent");
            return false;
        }

        var payload = message.ToByteArray();
        var frame = new byte[4 + payload.Length];
        BinaryPrimitives.WriteUInt32LittleEndian(frame.AsSpan(0, 4), (uint)payload.Length);
        payload.CopyTo(frame, 4);

        await _sendLock.WaitAsync();
        try
        {
            await stream.WriteAsync(frame);
            await stream.FlushAsync();
            Log?.Invoke($"sent {message.Descriptor.Name} {message}");
            return true;
        }
        catch (Exception ex) when (ex is IOException or ObjectDisposedException or SocketException)
        {
            Log?.Invoke($"send failed: {ex.Message}");
            return false;
        }
        finally
        {
            _sendLock.Release();
        }
    }

    private async Task AcceptLoopAsync(CancellationToken token)
    {
        while (!token.IsCancellationRequested)
        {
            TcpClient client;
            try
            {
                client = await _listener!.AcceptTcpClientAsync(token);
            }
            catch (OperationCanceledException)
            {
                break;
            }
            catch (Exception ex) when (ex is ObjectDisposedException or SocketException)
            {
                break;
            }

            using (client)
            {
                _stream = client.GetStream();
                Connected?.Invoke(client.Client.RemoteEndPoint);

                try
                {
                    await ReceiveLoopAsync(_stream, token);
                }
                finally
                {
                    _stream = null;
                    Disconnected?.Invoke();
                }
            }

            // 1 台の projector だけを相手にする。切れたらまた accept に戻る。
        }
    }

    private async Task ReceiveLoopAsync(NetworkStream stream, CancellationToken token)
    {
        while (!token.IsCancellationRequested)
        {
            byte[]? frame;
            try
            {
                frame = await ReadFrameAsync(stream, token);
            }
            catch (OperationCanceledException)
            {
                return;
            }
            catch (Exception ex) when (ex is IOException or ObjectDisposedException or SocketException or InvalidDataException)
            {
                Log?.Invoke($"receive failed: {ex.Message}");
                return;
            }

            if (frame is null) return;

            FrameReceived?.Invoke(frame);
        }
    }

    /// <summary>1 フレーム読む。相手がきれいに閉じた場合は null。</summary>
    private static async Task<byte[]?> ReadFrameAsync(NetworkStream stream, CancellationToken token)
    {
        var header = new byte[4];
        if (!await ReadExactAsync(stream, header, token)) return null;

        uint length = BinaryPrimitives.ReadUInt32LittleEndian(header);
        if (length > MaxFrameBytes)
        {
            throw new InvalidDataException($"frame length {length} exceeds the {MaxFrameBytes} byte limit");
        }

        var payload = new byte[length];
        if (length > 0 && !await ReadExactAsync(stream, payload, token)) return null;
        return payload;
    }

    private static async Task<bool> ReadExactAsync(NetworkStream stream, byte[] buffer, CancellationToken token)
    {
        int offset = 0;
        while (offset < buffer.Length)
        {
            int read = await stream.ReadAsync(buffer.AsMemory(offset), token);
            if (read == 0) return false;
            offset += read;
        }
        return true;
    }

    public async ValueTask DisposeAsync()
    {
        await StopAsync();
        _sendLock.Dispose();
    }
}
