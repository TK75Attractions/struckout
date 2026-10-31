using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Diagnostics;
using Avalonia.Threading;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Struckout.Launcher.Core;
using Struckout.Launcher.Core.Install;
using Struckout.Launcher.Core.Launch;
using Struckout.Launcher.Core.Modules;
using Struckout.Launcher.Core.Platform;
using Struckout.Launcher.Core.Spec;

namespace Struckout.Launcher.ViewModels;

/// <summary>A module on this PC on the launch screen.</summary>
public sealed partial class ModuleStatusViewModel(ModuleId id) : ObservableObject
{
    public ModuleId Id { get; } = id;

    public string Name => Id.Slug();

    [ObservableProperty]
    public partial string State { get; private set; } = "";

    [ObservableProperty]
    public partial string Message { get; private set; } = "";

    [ObservableProperty]
    public partial bool IsOk { get; private set; }

    [ObservableProperty]
    public partial bool IsError { get; private set; }

    public void Update(ModuleStatus status)
    {
        State = status.State switch
        {
            ModuleState.Stopped => "停止",
            ModuleState.Waiting => "待機中",
            ModuleState.Starting => "起動中",
            ModuleState.Running => "稼働中",
            ModuleState.Unhealthy => "異常",
            ModuleState.Failed => "失敗",
            _ => status.State.ToString(),
        };
        Message = status.Message;
        IsOk = status.State == ModuleState.Running;
        IsError = status.State is ModuleState.Unhealthy or ModuleState.Failed;
    }
}

/// <summary>A module on another PC that this PC connects to.</summary>
public sealed record RemoteStatusViewModel(string Name, string Host, string Message, bool IsOk)
{
    public bool IsError => !IsOk;
}

/// <summary>The launch screen: start and stop the modules of this PC and watch them.</summary>
public sealed partial class LaunchViewModel : ObservableObject, IDisposable
{
    private static readonly TimeSpan CheckInterval = TimeSpan.FromSeconds(2);

    private readonly HostPlatform platform;
    private readonly AppPaths paths;
    private readonly ServerPorts ports;
    private readonly ICommandRunner runner;
    private readonly DispatcherTimer timer;
    private Supervisor? supervisor;
    private bool checking;

    public LaunchViewModel(HostPlatform platform, AppPaths paths, ServerPorts ports, ICommandRunner runner)
    {
        this.platform = platform;
        this.paths = paths;
        this.ports = ports;
        this.runner = runner;
        timer = new DispatcherTimer { Interval = CheckInterval };
        timer.Tick += async (_, _) => await CheckAsync();
        CameraAssignment = new CameraAssignmentViewModel(platform, paths, runner, Log);
    }

    public ObservableCollection<ModuleStatusViewModel> Modules { get; } = [];

    public ObservableCollection<RemoteStatusViewModel> Remotes { get; } = [];

    public LogViewModel Log { get; } = new();

    public CameraAssignmentViewModel CameraAssignment { get; }

    public string ConfigPath => paths.LauncherConfigFile;

    [ObservableProperty]
    public partial string ConfigError { get; private set; } = "";

    [ObservableProperty]
    [NotifyCanExecuteChangedFor(nameof(StartCommand), nameof(StopCommand), nameof(ReloadCommand))]
    [NotifyPropertyChangedFor(nameof(CanReload))]
    public partial bool IsBusy { get; private set; }

    /// <summary>Some modules were started and not stopped yet.</summary>
    [ObservableProperty]
    [NotifyCanExecuteChangedFor(nameof(StartCommand), nameof(StopCommand), nameof(ReloadCommand))]
    [NotifyPropertyChangedFor(nameof(CanReload))]
    public partial bool IsRunning { get; private set; }

    public bool CanReload => !IsBusy && !IsRunning;

    partial void OnIsBusyChanged(bool value) => CameraAssignment.IsLocked = !CanReload;

    partial void OnIsRunningChanged(bool value) => CameraAssignment.IsLocked = !CanReload;

    /// <summary>Reads <c>launcher.toml</c> and the install state again. Not while modules are running.</summary>
    [RelayCommand(CanExecute = nameof(CanReload))]
    public void Reload()
    {
        supervisor?.Dispose();
        supervisor = null;
        Modules.Clear();
        Remotes.Clear();
        try
        {
            var config = LauncherConfig.Load(paths.LauncherConfigFile);
            var state = InstallState.Load(paths.InstallStateFile);
            supervisor = new Supervisor(config, state, paths, platform, ports, runner);
            supervisor.StatusChanged += OnStatusChanged;
            foreach (var status in supervisor.Statuses)
            {
                var module = new ModuleStatusViewModel(status.Id);
                module.Update(status);
                Modules.Add(module);
            }

            ConfigError = supervisor.Order.Count == 0 ? "modules にこの PC で起動するモジュールを書いてください" : "";
            timer.Start();
            if (supervisor.Order.Contains(ModuleId.Camera))
            {
                // It handles its own errors.
                _ = CameraAssignment.RefreshAsync();
            }
            else
            {
                CameraAssignment.Clear();
            }
        }
        catch (LauncherException e)
        {
            ConfigError = e.Message;
        }

        StartCommand.NotifyCanExecuteChanged();
    }

    private bool CanStart() => !IsBusy && supervisor is { Order.Count: > 0 };

    [RelayCommand(CanExecute = nameof(CanStart))]
    private async Task StartAsync()
    {
        if (supervisor is null)
        {
            return;
        }

        IsBusy = true;
        IsRunning = true;
        try
        {
            Log.Append($"{string.Join(" → ", supervisor.Order.Select(m => m.Slug()))} の順に起動します");
            await supervisor.StartAllAsync(Log.CreateProgress(), CancellationToken.None);
            Log.Append("起動の手順が終わりました");
        }
        finally
        {
            IsBusy = false;
        }
    }

    private bool CanStop() => !IsBusy && IsRunning;

    [RelayCommand(CanExecute = nameof(CanStop))]
    private Task StopAsync() => ShutdownAsync();

    /// <summary>Stops everything this launcher started. Also called when the window closes.</summary>
    public async Task ShutdownAsync()
    {
        if (supervisor is null || !IsRunning)
        {
            return;
        }

        IsBusy = true;
        try
        {
            await supervisor.StopAllAsync(Log.CreateProgress(), CancellationToken.None);
            Log.Append("止めました");
            IsRunning = false;
        }
        finally
        {
            IsBusy = false;
        }
    }

    [RelayCommand]
    private void OpenConfig() => Open(paths.LauncherConfigFile);

    [RelayCommand]
    private void OpenLogs()
    {
        Directory.CreateDirectory(paths.LogsDirectory);
        Open(paths.LogsDirectory);
    }

    public void Dispose()
    {
        timer.Stop();
        supervisor?.Dispose();
    }

    private void Open(string path)
    {
        try
        {
            // The OS opens it with the default app (xdg-open on Linux, open on macOS).
            Process.Start(new ProcessStartInfo(path) { UseShellExecute = true })?.Dispose();
        }
        catch (Win32Exception e)
        {
            Log.Append($"{path} を開けません: {e.Message}");
        }
    }

    private async Task CheckAsync()
    {
        if (supervisor is null || checking)
        {
            return;
        }

        checking = true;
        try
        {
            await supervisor.CheckAllAsync(CancellationToken.None);
            var remotes = await supervisor.CheckRemotesAsync(CancellationToken.None);
            Remotes.Clear();
            foreach (var remote in remotes)
            {
                Remotes.Add(new RemoteStatusViewModel(
                    remote.Id.Slug(), remote.Host, remote.Health.Message, remote.Health.Kind == HealthKind.Up));
            }
        }
        catch (LauncherException e)
        {
            // adb failing while a phone is plugged out, ...: shown and retried on the next tick
            Log.Append(e.Message);
        }
        finally
        {
            checking = false;
        }
    }

    private void OnStatusChanged(object? sender, ModuleStatus status) =>
        Dispatcher.UIThread.Post(() =>
        {
            Modules.FirstOrDefault(m => m.Id == status.Id)?.Update(status);
            if (status.State is ModuleState.Failed or ModuleState.Unhealthy)
            {
                Log.Append($"{status.Id.Slug()}: {status.Message}");
            }
        });
}
