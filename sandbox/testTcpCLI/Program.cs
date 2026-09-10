using System.Globalization;
using Google.Protobuf;
using Struckout.TestCli;
using Tk75Attractions.Struckout.V1;

// projector を単体でデバッグするためのダミー対向サーバ。
//
//   sensor : ball_tracker のかわり (既定 5000) -> ProjectorPacket を送る
//
// game_master は gRPC になったので、その対向はここには無い。
// 偽 game_master は別途 gRPC サーバとして用意する。

const int DefaultSensorPort = 5000;

// CollisionPoint は物理座標 (m)。ball_tracker は三角測量の結果をそのまま送る
// (ball_tracker/src/collision_output/network.rs: x = coll.x, y = coll.z)。
// ランダム送信で使う範囲の想定値で、盤面の実寸が決まったら直すこと。
const double FieldMinX = -1.0;
const double FieldMaxX = 1.0;
const double FieldMinY = 0.0;
const double FieldMaxY = 2.0;

await using var sensor = new DummyPeer("sensor");

// sensor は projector から何も受け取らない想定なので、来たら異常として報告する。
Wire(sensor, frame => $"WARNING: received {frame.Length} byte(s), but nothing is expected on this channel");

void Wire(DummyPeer peer, Func<byte[], string> describe)
{
    peer.Log += message => ConsoleLog.Write(peer.Name, message);
    peer.Connected += endPoint => ConsoleLog.Write(peer.Name, $"projector connected from {endPoint}");
    peer.Disconnected += () => ConsoleLog.Write(peer.Name, "projector disconnected");
    peer.FrameReceived += frame =>
    {
        try
        {
            ConsoleLog.Write(peer.Name, describe(frame));
        }
        catch (InvalidProtocolBufferException ex)
        {
            ConsoleLog.Write(peer.Name, $"received {frame.Length} byte(s) that failed to parse: {ex.Message}");
        }
    };
}

var random = new Random();
CancellationTokenSource? autoCts = null;
Task? autoTask = null;

PrintHelp();

bool running = true;
while (running)
{
    Console.Write("> ");
    var line = Console.ReadLine();
    if (line is null) break;

    // Windows でコマンドをパイプで流し込むと先頭に UTF-8 BOM が付くことがある。
    line = line.TrimStart('\uFEFF');
    if (string.IsNullOrWhiteSpace(line)) continue;

    var tokens = line.Split(' ', StringSplitOptions.RemoveEmptyEntries);

    try
    {
        switch (tokens[0].ToLowerInvariant())
        {
            case "listen":
                await HandleListen(tokens);
                break;
            case "stop":
                await HandleStop(tokens);
                break;
            case "hit":
                await HandleHit(tokens);
                break;
            case "msg":
                await HandleMsg(tokens, line);
                break;
            case "auto":
                await HandleAuto(tokens);
                break;
            case "status":
                PrintStatus();
                break;
            case "help":
                PrintHelp();
                break;
            case "exit":
            case "quit":
                running = false;
                break;
            default:
                ConsoleLog.Plain($"unknown command '{tokens[0]}'. type 'help'.");
                break;
        }
    }
    catch (Exception ex)
    {
        ConsoleLog.Plain($"ERROR: {ex.Message}");
    }
}

await StopAuto();
ConsoleLog.Plain("bye");
return 0;

async Task HandleListen(string[] tokens)
{
    if (tokens.Length < 2)
    {
        ConsoleLog.Plain("usage: listen sensor [port]");
        return;
    }

    var (peer, defaultPort) = ResolvePeer(tokens[1]);
    var port = tokens.Length >= 3 ? ParsePort(tokens[2]) : defaultPort;
    peer.Listen(port);
    await Task.CompletedTask;
}

async Task HandleStop(string[] tokens)
{
    if (tokens.Length < 2)
    {
        ConsoleLog.Plain("usage: stop sensor");
        return;
    }

    var (peer, _) = ResolvePeer(tokens[1]);
    await peer.StopAsync();
}

async Task HandleHit(string[] tokens)
{
    double x;
    double y;

    if (tokens.Length >= 3)
    {
        x = ParseCoordinate(tokens[1]);
        y = ParseCoordinate(tokens[2]);
    }
    else
    {
        x = RandomInRange(FieldMinX, FieldMaxX);
        y = RandomInRange(FieldMinY, FieldMaxY);
    }

    await sensor.SendAsync(new ProjectorPacket
    {
        Point = new CollisionPoint { X = x, Y = y },
    });
}

async Task HandleMsg(string[] tokens, string line)
{
    if (tokens.Length < 2)
    {
        ConsoleLog.Plain("usage: msg <text>");
        return;
    }

    // コマンド名だけを落として、残りは空白ごと本文として扱う。
    var text = line[(line.IndexOf(tokens[0], StringComparison.Ordinal) + tokens[0].Length)..].Trim();

    await sensor.SendAsync(new ProjectorPacket
    {
        Message = new TestMessage { Message = text },
    });
}

async Task HandleAuto(string[] tokens)
{
    if (tokens.Length < 2)
    {
        ConsoleLog.Plain("usage: auto <intervalMs> | auto off");
        return;
    }

    if (tokens[1].Equals("off", StringComparison.OrdinalIgnoreCase))
    {
        await StopAuto();
        ConsoleLog.Plain("auto fire stopped");
        return;
    }

    if (!int.TryParse(tokens[1], NumberStyles.Integer, CultureInfo.InvariantCulture, out var intervalMs) || intervalMs <= 0)
    {
        throw new ArgumentException($"'{tokens[1]}' is not a positive interval in milliseconds");
    }

    await StopAuto();

    autoCts = new CancellationTokenSource();
    autoTask = AutoFireAsync(intervalMs, autoCts.Token);
    ConsoleLog.Plain($"auto fire started ({intervalMs} ms)");
}

async Task AutoFireAsync(int intervalMs, CancellationToken token)
{
    while (!token.IsCancellationRequested)
    {
        try
        {
            await Task.Delay(intervalMs, token);
        }
        catch (OperationCanceledException)
        {
            return;
        }

        await sensor.SendAsync(new ProjectorPacket
        {
            Point = new CollisionPoint
            {
                X = RandomInRange(FieldMinX, FieldMaxX),
                Y = RandomInRange(FieldMinY, FieldMaxY),
            },
        });
    }
}

double RandomInRange(double min, double max) => min + random.NextDouble() * (max - min);

async Task StopAuto()
{
    if (autoCts is null) return;

    autoCts.Cancel();
    if (autoTask is not null)
    {
        try
        {
            await autoTask;
        }
        catch (OperationCanceledException)
        {
            // 想定内
        }
    }

    autoCts.Dispose();
    autoCts = null;
    autoTask = null;
}

(DummyPeer Peer, int DefaultPort) ResolvePeer(string name) => name.ToLowerInvariant() switch
{
    "sensor" => (sensor, DefaultSensorPort),
    _ => throw new ArgumentException($"unknown peer '{name}' (sensor)"),
};

static int ParsePort(string value)
{
    if (!int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out var port) || port is < 1 or > 65535)
    {
        throw new ArgumentException($"'{value}' is not a valid port");
    }
    return port;
}

static double ParseCoordinate(string value)
{
    if (!double.TryParse(value, NumberStyles.Float, CultureInfo.InvariantCulture, out var result))
    {
        throw new ArgumentException($"'{value}' is not a number");
    }
    return result;
}

void PrintStatus()
{
    ConsoleLog.Plain($"sensor : {Describe(sensor)}");
    ConsoleLog.Plain($"auto   : {(autoTask is null ? "off" : "on")}");

    static string Describe(DummyPeer peer)
    {
        if (!peer.IsListening) return "not listening";
        return peer.IsConnected
            ? $"listening on {peer.Port}, projector connected"
            : $"listening on {peer.Port}, waiting for projector";
    }
}

static void PrintHelp()
{
    ConsoleLog.Plain("""
        struckout dummy ball_tracker -- lets the Unity projector be debugged without
        the real tracker running.

          listen sensor [port]          start listening (default 5000)
          stop   sensor                 stop listening and drop the connection

          hit [x y]                     send a CollisionPoint in metres (no args = random)
          msg <text>                    send a TestMessage
          auto <intervalMs> | auto off  send random hits repeatedly

          status                        show connection state
          help                          show this text
          exit                          quit

        game_master speaks gRPC now, so its stand-in is a separate program.
        """);
}
