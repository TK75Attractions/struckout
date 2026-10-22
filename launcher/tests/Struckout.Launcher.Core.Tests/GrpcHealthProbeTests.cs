using System.Net;
using System.Net.Sockets;
using Grpc.Health.V1;
using Struckout.Launcher.Core.Launch;

namespace Struckout.Launcher.Core.Tests;

public class GrpcHealthProbeTests
{
    [Theory]
    [InlineData(HealthCheckResponse.Types.ServingStatus.Serving, HealthKind.Up)]
    [InlineData(HealthCheckResponse.Types.ServingStatus.NotServing, HealthKind.Down)]
    [InlineData(HealthCheckResponse.Types.ServingStatus.Unknown, HealthKind.Down)]
    [InlineData(HealthCheckResponse.Types.ServingStatus.ServiceUnknown, HealthKind.Down)]
    public void OnlyServingIsUp(HealthCheckResponse.Types.ServingStatus status, HealthKind expected)
    {
        var health = GrpcHealthProbe.FromStatus("localhost", 5080, status);

        Assert.Equal(expected, health.Kind);
        if (expected == HealthKind.Down)
        {
            Assert.Equal($"localhost:5080 は準備ができていません ({status})", health.Message);
        }
    }

    [Fact]
    public async Task ClosedPortIsDown()
    {
        using var listener = new TcpListener(IPAddress.Loopback, 0);
        listener.Start();
        var port = ((IPEndPoint)listener.LocalEndpoint).Port;
        listener.Stop();

        var health = await GrpcHealthProbe.CheckAsync("127.0.0.1", port, TestContext.Current.CancellationToken);

        Assert.Equal(HealthKind.Down, health.Kind);
        Assert.Equal($"127.0.0.1:{port} につながりません", health.Message);
    }

    [Fact]
    public async Task CallerCancellationIsPropagated()
    {
        using var cancellation = new CancellationTokenSource();
        await cancellation.CancelAsync();

        await Assert.ThrowsAnyAsync<OperationCanceledException>(() =>
            GrpcHealthProbe.CheckAsync("127.0.0.1", 5080, cancellation.Token));
    }
}
