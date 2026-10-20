using Struckout.Launcher.Core.Spec;

namespace Struckout.Launcher.Core.Modules;

/// <summary><paramref name="Target"/> has to accept connections on <paramref name="Ports"/>.</summary>
public sealed record Dependency(ModuleId Target, IReadOnlyList<int> Ports);

/// <summary>Which module connects to which, as drawn in <c>docs/architecture.mmd</c>.</summary>
public static class ModuleGraph
{
    public static IReadOnlyList<Dependency> DependenciesOf(ModuleId id, ServerPorts ports) => id switch
    {
        ModuleId.Camera => [new(ModuleId.BallTracker, [ports.BallTrackerCameraControl, ports.BallTrackerCameraData])],
        ModuleId.Projector =>
        [
            new(ModuleId.BallTracker, [ports.BallTrackerProjector]),
            new(ModuleId.GameMaster, [ports.GameMasterGrpc]),
        ],
        ModuleId.Touchpanel => [new(ModuleId.GameMaster, [ports.GameMasterGrpc])],
        ModuleId.BallTracker or ModuleId.GameMaster => [],
        _ => throw new ArgumentOutOfRangeException(nameof(id), id, null),
    };

    /// <summary>The ports that are open while the module is up. Empty for the modules that only connect.</summary>
    public static IReadOnlyList<int> PortsServedBy(ModuleId id, ServerPorts ports) => id switch
    {
        ModuleId.BallTracker =>
            [ports.BallTrackerProjector, ports.BallTrackerCameraControl, ports.BallTrackerCameraData],
        ModuleId.GameMaster => [ports.GameMasterGrpc],
        ModuleId.Camera or ModuleId.Projector or ModuleId.Touchpanel => [],
        _ => throw new ArgumentOutOfRangeException(nameof(id), id, null),
    };

    /// <summary>
    /// Sorts <paramref name="modules"/> so that each comes after the modules it depends on.
    /// Dependencies outside <paramref name="modules"/> (on another PC) are ignored.
    /// </summary>
    public static IReadOnlyList<ModuleId> StartOrder(IEnumerable<ModuleId> modules, ServerPorts ports)
    {
        var remaining = modules.Distinct().Order().ToList();
        var order = new List<ModuleId>();
        while (remaining.Count > 0)
        {
            var ready = remaining
                .Where(id => DependenciesOf(id, ports).All(d => !remaining.Contains(d.Target)))
                .ToList();
            // guaranteed by DependenciesOf: the graph is a fixed DAG
            if (ready.Count == 0)
            {
                throw new InvalidOperationException("module dependencies should not have a cycle");
            }

            order.AddRange(ready);
            remaining.RemoveAll(ready.Contains);
        }

        return order;
    }
}
