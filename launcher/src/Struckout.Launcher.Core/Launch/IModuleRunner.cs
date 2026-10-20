using Struckout.Launcher.Core.Modules;

namespace Struckout.Launcher.Core.Launch;

/// <summary>Starts, watches and stops one module on this PC.</summary>
internal interface IModuleRunner
{
    ModuleId Id { get; }

    /// <summary>Returns once the module is launched, not when it's ready (see <see cref="CheckAsync"/>).</summary>
    /// <exception cref="LauncherException">It can't be started.</exception>
    Task StartAsync(IProgress<string> log, CancellationToken cancellationToken);

    Task<Health> CheckAsync(CancellationToken cancellationToken);

    /// <exception cref="LauncherException">It can't be stopped.</exception>
    Task StopAsync(IProgress<string> log, CancellationToken cancellationToken);
}
