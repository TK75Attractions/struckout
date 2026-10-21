using System.Net.Sockets;
using Grpc.Core;
using Grpc.Health.V1;
using Grpc.Net.Client;

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

/// <summary>Checks readiness reported by the module, rather than just an open port.</summary>
public static class GrpcHealthProbe
{
    public static async Task<Health> CheckAsync(string host, int port, CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        using var timeout = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        timeout.CancelAfter(TimeSpan.FromSeconds(1));
        using var channel = GrpcChannel.ForAddress(new UriBuilder("http", host, port).Uri,
            new GrpcChannelOptions { ThrowOperationCanceledOnCancellation = true });
        var client = new Grpc.Health.V1.Health.HealthClient(channel);
        try
        {
            using var call = client.CheckAsync(new HealthCheckRequest { Service = "" }, cancellationToken: timeout.Token);
            var response = await call.ResponseAsync;
            return FromStatus(host, port, response.Status);
        }
        catch (RpcException)
        {
            cancellationToken.ThrowIfCancellationRequested();
            return Health.Down($"{host}:{port} につながりません");
        }
        catch (OperationCanceledException) when (!cancellationToken.IsCancellationRequested)
        {
            return Health.Down($"{host}:{port} につながりません");
        }
    }

    internal static Health FromStatus(string host, int port, HealthCheckResponse.Types.ServingStatus status) =>
        status == HealthCheckResponse.Types.ServingStatus.Serving
            ? Health.Up()
            : Health.Down($"{host}:{port} は準備ができていません ({status})");
}

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
}
