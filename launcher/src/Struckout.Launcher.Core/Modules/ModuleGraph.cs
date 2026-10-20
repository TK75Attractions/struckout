using Struckout.Launcher.Core.Spec;

namespace Struckout.Launcher.Core.Modules;

/// <summary><paramref name="Target"/> must report SERVING on <paramref name="HealthPort"/> before its dependents start.</summary>
public sealed record Dependency(ModuleId Target, int HealthPort);

/// <summary>Which module connects to which, as drawn in <c>docs/architecture.mmd</c>.</summary>
public static class ModuleGraph
{
    public static IReadOnlyList<Dependency> DependenciesOf(ModuleId id, ServerPorts ports) => id switch
    {
        ModuleId.Camera => [DependencyOf(ModuleId.BallTracker, ports)],
        ModuleId.Projector =>
        [
            DependencyOf(ModuleId.BallTracker, ports),
            DependencyOf(ModuleId.GameMaster, ports),
        ],
        ModuleId.Touchpanel => [DependencyOf(ModuleId.GameMaster, ports)],
        ModuleId.BallTracker or ModuleId.GameMaster => [],
        _ => throw new ArgumentOutOfRangeException(nameof(id), id, null),
    };

    /// <summary>The gRPC health port for readiness checks. Client-only modules have none.</summary>
    public static int? HealthPortOf(ModuleId id, ServerPorts ports) => id switch
    {
        ModuleId.GameMaster => ports.GameMasterGrpc,
        ModuleId.BallTracker => ports.BallTrackerHealth,
        ModuleId.Camera or ModuleId.Projector or ModuleId.Touchpanel => null,
        _ => throw new ArgumentOutOfRangeException(nameof(id), id, null),
    };

    private static Dependency DependencyOf(ModuleId target, ServerPorts ports) =>
        new(target, HealthPortOf(target, ports)
            // guaranteed by DependenciesOf: only server modules are dependencies
            ?? throw new InvalidOperationException("a dependency should have a health endpoint"));

    /// <summary>Ports checked for conflicts before starting a module. Empty for client-only modules.</summary>
    public static IReadOnlyList<int> PortsServedBy(ModuleId id, ServerPorts ports) => id switch
    {
        ModuleId.BallTracker =>
            [ports.BallTrackerProjector, ports.BallTrackerCameraControl, ports.BallTrackerCameraData, ports.BallTrackerHealth],
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
