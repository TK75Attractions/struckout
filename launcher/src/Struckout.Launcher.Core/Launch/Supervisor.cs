using System.Globalization;
using Struckout.Launcher.Core.Install;
using Struckout.Launcher.Core.Modules;
using Struckout.Launcher.Core.Platform;
using Struckout.Launcher.Core.Spec;

namespace Struckout.Launcher.Core.Launch;

public enum ModuleState
{
    Stopped,

    /// <summary>Waiting for a module on this PC that it depends on.</summary>
    Waiting,

    /// <summary>Launched but not ready yet.</summary>
    Starting,
    Running,

    /// <summary>It was running and stopped working, or it ended.</summary>
    Unhealthy,

    /// <summary>It couldn't be started.</summary>
    Failed,
}

public sealed record ModuleStatus(ModuleId Id, ModuleState State, string Message);

/// <summary>A module on another PC that a module on this PC connects to.</summary>
public sealed record RemoteStatus(ModuleId Id, string Host, Health Health);

/// <summary>Starts the modules of <see cref="LauncherConfig.Modules"/> in dependency order and watches them.</summary>
public sealed class Supervisor : IDisposable
{
    /// <summary>
    /// game_master creates its database on the first start, which takes about a minute.
    /// </summary>
    private static readonly TimeSpan DependencyTimeout = TimeSpan.FromMinutes(3);

    private static readonly TimeSpan PollInterval = TimeSpan.FromSeconds(1);

    private readonly LauncherConfig config;
    private readonly ServerPorts ports;
    private readonly Dictionary<ModuleId, IModuleRunner> runners = [];
    private readonly Dictionary<ModuleId, ModuleStatus> statuses = [];
    private readonly HashSet<ModuleId> launched = [];

    // Starting, stopping and checking run one at a time.
    private readonly SemaphoreSlim gate = new(1, 1);

    public Supervisor(
        LauncherConfig config, InstallState state, AppPaths paths, HostPlatform platform, ServerPorts ports, ICommandRunner runner)
    {
        this.config = config;
        this.ports = ports;
        Order = ModuleGraph.StartOrder(config.Modules, ports);
        var locator = new ToolLocator(platform, paths);
        var logs = Path.Combine(paths.LogsDirectory, DateTime.Now.ToString("yyyy-MM-dd", CultureInfo.InvariantCulture));
        var time = DateTime.Now.ToString("HHmmss", CultureInfo.InvariantCulture);
        foreach (var id in Order)
        {
            var logFile = Path.Combine(logs, $"{id.Slug()}_{time}.log");
            try
            {
                runners[id] = CreateRunner(id, state, paths, platform, locator, runner, logFile);
                statuses[id] = new ModuleStatus(id, ModuleState.Stopped, "止まっています");
            }
            catch (LauncherException e)
            {
                statuses[id] = new ModuleStatus(id, ModuleState.Failed, e.Message);
            }
        }
    }

    /// <summary>Raised on any thread when a module's state or message changes.</summary>
    public event EventHandler<ModuleStatus>? StatusChanged;

    /// <summary>The modules of this PC in the order they're started.</summary>
    public IReadOnlyList<ModuleId> Order { get; }

    public IReadOnlyList<ModuleStatus> Statuses
    {
        get
        {
            lock (statuses)
            {
                return Order.Select(id => statuses[id]).ToList();
            }
        }
    }

    /// <summary>
    /// Starts the modules one by one. A module waits until the modules on this PC it depends on are up,
    /// and isn't started if they don't come up. Modules on other PCs aren't waited for: every module
    /// reconnects by itself.
    /// </summary>
    public async Task StartAllAsync(IProgress<string> log, CancellationToken cancellationToken)
    {
        await gate.WaitAsync(cancellationToken);
        try
        {
            await StartAllUnlockedAsync(log, cancellationToken);
        }
        finally
        {
            gate.Release();
        }
    }

    private async Task StartAllUnlockedAsync(IProgress<string> log, CancellationToken cancellationToken)
    {
        foreach (var id in Order)
        {
            if (!runners.TryGetValue(id, out var runner) || launched.Contains(id))
            {
                continue;
            }

            var blocker = await WaitForDependenciesAsync(id, cancellationToken);
            if (blocker is not null)
            {
                Update(id, ModuleState.Failed, blocker);
                continue;
            }

            Update(id, ModuleState.Starting, "起動しています");
            try
            {
                await runner.StartAsync(log, cancellationToken);
                launched.Add(id);
                await CheckAsync(id, cancellationToken);
            }
            catch (LauncherException e)
            {
                Update(id, ModuleState.Failed, e.Message);
            }
        }
    }

    /// <summary>Stops the modules in the reverse order they were started.</summary>
    public async Task StopAllAsync(IProgress<string> log, CancellationToken cancellationToken)
    {
        await gate.WaitAsync(cancellationToken);
        try
        {
            await StopAllUnlockedAsync(log, cancellationToken);
        }
        finally
        {
            gate.Release();
        }
    }

    private async Task StopAllUnlockedAsync(IProgress<string> log, CancellationToken cancellationToken)
    {
        foreach (var id in Order.Reverse())
        {
            if (!launched.Contains(id))
            {
                continue;
            }

            try
            {
                await runners[id].StopAsync(log, cancellationToken);
                launched.Remove(id);
                Update(id, ModuleState.Stopped, "止まっています");
            }
            catch (LauncherException e)
            {
                Update(id, ModuleState.Unhealthy, e.Message);
            }
        }
    }

    /// <summary>
    /// Updates the states of the launched modules. Call it periodically.
    /// It does nothing while the modules are being started or stopped, which update the states themselves.
    /// </summary>
    public async Task CheckAllAsync(CancellationToken cancellationToken)
    {
        if (!await gate.WaitAsync(0, cancellationToken))
        {
            return;
        }

        try
        {
            foreach (var id in launched.ToList())
            {
                await CheckAsync(id, cancellationToken);
            }
        }
        finally
        {
            gate.Release();
        }
    }

    /// <summary>Whether the modules on other PCs that this PC uses are reachable.</summary>
    public async Task<IReadOnlyList<RemoteStatus>> CheckRemotesAsync(CancellationToken cancellationToken)
    {
        var remotes = config.Modules
            .SelectMany(id => ModuleGraph.DependenciesOf(id, ports))
            .Where(d => !config.Modules.Contains(d.Target))
            .GroupBy(d => d.Target)
            .Select(g => (Target: g.Key, Ports: g.SelectMany(d => d.Ports).Distinct().Order().ToList()))
            .OrderBy(r => r.Target)
            .ToList();
        var results = new List<RemoteStatus>();
        foreach (var (target, targetPorts) in remotes)
        {
            var host = config.HostOf(target);
            results.Add(new RemoteStatus(target, host, await TcpProbe.CheckAsync(host, targetPorts, cancellationToken)));
        }

        return results;
    }

    /// <returns>Why <paramref name="id"/> can't be started, or null when its dependencies are up.</returns>
    private async Task<string?> WaitForDependenciesAsync(ModuleId id, CancellationToken cancellationToken)
    {
        var local = ModuleGraph.DependenciesOf(id, ports).Where(d => config.Modules.Contains(d.Target)).ToList();
        if (local.Count == 0)
        {
            return null;
        }

        Update(id, ModuleState.Waiting, $"{string.Join("、", local.Select(d => d.Target.Slug()))} を待っています");
        var deadline = DateTimeOffset.UtcNow + DependencyTimeout;
        foreach (var dependency in local)
        {
            while (true)
            {
                var state = StatusOf(dependency.Target).State;
                if (state is ModuleState.Failed or ModuleState.Stopped)
                {
                    return $"{dependency.Target.Slug()} が起動していないので起動しません";
                }

                if ((await TcpProbe.CheckAsync("127.0.0.1", dependency.Ports, cancellationToken)).Kind == HealthKind.Up)
                {
                    break;
                }

                if (DateTimeOffset.UtcNow > deadline)
                {
                    return $"{dependency.Target.Slug()} が {DependencyTimeout.TotalMinutes} 分たっても応答しないので起動しません";
                }

                await Task.Delay(PollInterval, cancellationToken);
                await CheckAsync(dependency.Target, cancellationToken);
            }
        }

        return null;
    }

    private async Task CheckAsync(ModuleId id, CancellationToken cancellationToken)
    {
        if (!launched.Contains(id))
        {
            return;
        }

        var health = await runners[id].CheckAsync(cancellationToken);
        var previous = StatusOf(id).State;
        var state = health.Kind switch
        {
            HealthKind.Up => ModuleState.Running,
            HealthKind.Exited => ModuleState.Unhealthy,
            // Not ready yet, unless it was already up
            HealthKind.Down => previous is ModuleState.Running or ModuleState.Unhealthy
                ? ModuleState.Unhealthy
                : ModuleState.Starting,
            _ => throw new InvalidOperationException($"unknown health {health.Kind}"),
        };
        if (health.Kind == HealthKind.Exited)
        {
            launched.Remove(id);
        }

        Update(id, state, health.Message);
    }

    private ModuleStatus StatusOf(ModuleId id)
    {
        lock (statuses)
        {
            return statuses[id];
        }
    }

    private void Update(ModuleId id, ModuleState state, string message)
    {
        var status = new ModuleStatus(id, state, message);
        lock (statuses)
        {
            if (statuses[id] == status)
            {
                return;
            }

            statuses[id] = status;
        }

        StatusChanged?.Invoke(this, status);
    }

    /// <summary>Releases the lock. It doesn't stop the modules: call <see cref="StopAllAsync"/> first.</summary>
    public void Dispose() => gate.Dispose();

    /// <exception cref="LauncherException">The module or a tool it needs isn't installed.</exception>
    private IModuleRunner CreateRunner(
        ModuleId id,
        InstallState state,
        AppPaths paths,
        HostPlatform platform,
        ToolLocator locator,
        ICommandRunner runner,
        string logFile)
    {
        if (!state.Modules.TryGetValue(id, out var installation))
        {
            throw new LauncherException("インストールされていません");
        }

        string Tool(PrerequisiteId prerequisite) =>
            locator.Find(prerequisite) ?? throw new LauncherException(
                $"{PrerequisiteInstaller.DisplayName(prerequisite)} が見つかりません。インストールし直してください");

        var machineId = config.MachineId.ToString(CultureInfo.InvariantCulture);
        switch (id, installation)
        {
            case (ModuleId.BallTracker, ExecutableInstallation executable):
                var output = config.BallTracker.Output switch
                {
                    CollisionOutput.Network => new[] { "--output", "network" },
                    CollisionOutput.Csv => ["--output", "csv", "--csv_path", config.BallTracker.CsvPath!],
                    _ => throw new InvalidOperationException($"unknown output {config.BallTracker.Output}"),
                };
                return new ProcessModuleRunner(
                    id, platform, runner, executable.Executable,
                    ["--input", "network", .. output, "--cameras", paths.CamerasFile, "--ffmpeg", Tool(PrerequisiteId.Ffmpeg),
                        .. config.BallTracker.ExtraArguments],
                    ModuleGraph.PortsServedBy(id, ports),
                    logFile);
            case (ModuleId.Touchpanel, ExecutableInstallation executable):
                return new ProcessModuleRunner(
                    id, platform, runner, executable.Executable,
                    ["--address", config.HostOf(ModuleId.GameMaster), "--machine-id", machineId],
                    [],
                    logFile);
            case (ModuleId.Projector, ExecutableInstallation executable):
                // The arguments are read by Infrastructure/NetworkSettingsResolver.cs and CollisionTransformStore.cs
                // of projector; -monitor, -logFile and -screen-fullscreen are Unity's.
                List<string> arguments =
                [
                    "-trackerHost", config.HostOf(ModuleId.BallTracker),
                    "-masterHost", config.HostOf(ModuleId.GameMaster),
                    "-machineId", machineId,
                    // projector gives up after this many attempts; keep it waiting for the other PC
                    "-connectAttempts", "100000",
                    "-monitor", config.Projector.Monitor.ToString(CultureInfo.InvariantCulture),
                    "-screen-fullscreen", config.Projector.Fullscreen ? "1" : "0",
                    "-logFile", logFile,
                ];
                if (config.Projector.CollisionTransform is { } transform)
                {
                    arguments.AddRange(["-collisionTransform", transform]);
                }

                return new ProcessModuleRunner(id, platform, runner, executable.Executable, arguments, [], null);
            case (ModuleId.GameMaster, ComposeInstallation compose):
                var password = GameMasterSecret.Load(paths)
                    ?? throw new LauncherException("データベースのパスワードがありません。インストールし直してください");
                return new GameMasterRunner(
                    new GameMasterCompose(
                        platform, Tool(PrerequisiteId.Docker), compose.Directory, compose.ReleaseTag, password, runner),
                    ports.GameMasterGrpc);
            case (ModuleId.Camera, ApkInstallation):
                return new CameraRunner(new Adb(Tool(PrerequisiteId.Adb), runner), paths.CamerasFile, ports);
            default:
                throw new LauncherException($"インストールの記録 ({installation.GetType().Name}) が壊れています");
        }
    }
}
