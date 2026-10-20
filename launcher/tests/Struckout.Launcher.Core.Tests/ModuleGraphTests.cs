using Struckout.Launcher.Core.Modules;
using Struckout.Launcher.Core.Spec;

namespace Struckout.Launcher.Core.Tests;

public class ModuleGraphTests
{
    public static TheoryData<ModuleId[], ModuleId[]> Orders => new()
    {
        // PC① of the booth
        { [ModuleId.Camera, ModuleId.BallTracker], [ModuleId.BallTracker, ModuleId.Camera] },
        // PC②: ball_tracker is on PC①, so projector waits only for game_master
        { [ModuleId.Projector, ModuleId.GameMaster], [ModuleId.GameMaster, ModuleId.Projector] },
        {
            [ModuleId.Touchpanel, ModuleId.Projector, ModuleId.Camera, ModuleId.GameMaster, ModuleId.BallTracker],
            [ModuleId.BallTracker, ModuleId.GameMaster, ModuleId.Camera, ModuleId.Projector, ModuleId.Touchpanel]
        },
        { [], [] },
    };

    [Theory]
    [MemberData(nameof(Orders))]
    public void DependenciesStartFirst(ModuleId[] modules, ModuleId[] expected)
    {
        Assert.Equal(expected, ModuleGraph.StartOrder(modules, ServerPorts.Spec));
    }

    [Fact]
    public void EveryDependencyIsAServer()
    {
        foreach (var id in ModuleIds.All)
        {
            foreach (var dependency in ModuleGraph.DependenciesOf(id, ServerPorts.Spec))
            {
                Assert.Equal(ModuleGraph.HealthPortOf(dependency.Target, ServerPorts.Spec), dependency.HealthPort);
                Assert.Contains(dependency.HealthPort, ModuleGraph.PortsServedBy(dependency.Target, ServerPorts.Spec));
            }
        }
    }

    [Fact]
    public void HealthPortsMatchTheServers()
    {
        var ports = ServerPorts.Spec;

        Assert.Equal(ports.GameMasterGrpc, ModuleGraph.HealthPortOf(ModuleId.GameMaster, ports));
        Assert.Equal(ports.BallTrackerHealth, ModuleGraph.HealthPortOf(ModuleId.BallTracker, ports));
        Assert.Null(ModuleGraph.HealthPortOf(ModuleId.Projector, ports));
        Assert.Null(ModuleGraph.HealthPortOf(ModuleId.Touchpanel, ports));
        Assert.Null(ModuleGraph.HealthPortOf(ModuleId.Camera, ports));
        Assert.Equal(
            new Dependency(ModuleId.BallTracker, ports.BallTrackerHealth),
            Assert.Single(ModuleGraph.DependenciesOf(ModuleId.Camera, ports)));
    }

    [Fact]
    public void SlugsRoundTrip()
    {
        foreach (var id in ModuleIds.All)
        {
            Assert.True(ModuleIds.TryFromSlug(id.Slug(), out var parsed));
            Assert.Equal(id, parsed);
        }

        Assert.False(ModuleIds.TryFromSlug("ball_tracker", out _));
    }
}
