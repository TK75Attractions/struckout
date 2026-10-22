using Struckout.Launcher.Core.Spec;

namespace Struckout.Launcher.Core.Tests;

public class SpecTests
{
    [Fact]
    public void EmbeddedSpecHasEveryPort()
    {
        var ports = ServerPorts.Spec;

        Assert.Equal(8020, ports.GameMasterGrpc);
        Assert.Equal(5000, ports.BallTrackerProjector);
        Assert.Equal(6060, ports.BallTrackerCameraControl);
        Assert.Equal(5050, ports.BallTrackerCameraData);
        Assert.Equal(5052, ports.BallTrackerCameraVideo);
        Assert.Equal(5080, ports.BallTrackerHealth);
    }

    [Fact]
    public void MissingEndpointIsAnError()
    {
        const string yaml = """
            servers:
              game-master:
                grpc:
                  type: grpc
                  port: 8020
            """;

        var error = Assert.Throws<InvalidDataException>(() => ServerPorts.Parse(yaml));
        Assert.Contains("ball-tracker.projector", error.Message, StringComparison.Ordinal);
    }
}
