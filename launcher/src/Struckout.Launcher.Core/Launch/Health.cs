using System.Net.Sockets;

namespace Struckout.Launcher.Core.Launch;

public enum HealthKind
{
    /// <summary>It works and accepts connections.</summary>
    Up,

    /// <summary>It's running but not ready (or not reachable) yet.</summary>
    Down,

    /// <summary>The process ended.</summary>
    Exited,
}

public sealed record Health(HealthKind Kind, string Message)
{
    public static Health Up(string message = "動いています") => new(HealthKind.Up, message);

    public static Health Down(string message) => new(HealthKind.Down, message);

    public static Health Exited(string message) => new(HealthKind.Exited, message);
}

// TODO: Ask the modules themselves once they have health check endpoints (the gRPC health checking
// protocol for game_master). An open port only tells that the process is listening.

/// <summary>Whether a server accepts TCP connections.</summary>
public static class TcpProbe
{
    public static async Task<bool> IsOpenAsync(string host, int port, CancellationToken cancellationToken)
    {
        using var timeout = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        timeout.CancelAfter(TimeSpan.FromSeconds(1));
        using var client = new TcpClient();
        try
        {
            await client.ConnectAsync(host, port, timeout.Token);
            return true;
        }
        catch (SocketException)
        {
            return false;
        }
        catch (OperationCanceledException) when (!cancellationToken.IsCancellationRequested)
        {
            return false;
        }
    }

    /// <summary>Up if every port is open, otherwise Down naming the first closed one.</summary>
    public static async Task<Health> CheckAsync(string host, IReadOnlyList<int> ports, CancellationToken cancellationToken)
    {
        foreach (var port in ports)
        {
            if (!await IsOpenAsync(host, port, cancellationToken))
            {
                return Health.Down($"{host}:{port} につながりません");
            }
        }

        return Health.Up();
    }
}
