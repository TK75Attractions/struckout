using System.Globalization;
using System.Text.RegularExpressions;
using Struckout.Launcher.Core.Modules;
using Struckout.Launcher.Core.Spec;

namespace Struckout.Launcher.Core.Launch;

/// <summary>The camera app on the phones in <c>cameras.json</c> that are connected over adb.</summary>
/// <remarks>
/// The app is controlled with the broadcasts of <c>remote/RemoteControl.kt</c>, like <c>scripts/camera_remote.py</c>.
/// </remarks>
internal sealed partial class CameraRunner(Adb adb, string camerasFile, ServerPorts ports) : IModuleRunner
{
    private const string ActionPrefix = "com.taichi765.struckoutCameraApp.remote.";

    /// <summary>Opening both phones at once makes the high-speed capture stutter.</summary>
    private static readonly TimeSpan OpenInterval = TimeSpan.FromSeconds(3);

    private CamerasFile? cameras;
    private List<CameraEntry> started = [];

    public ModuleId Id => ModuleId.Camera;

    /// <summary>
    /// Forwards ball_tracker's ports to each phone, sends its settings, records the ID it reports and opens the app.
    /// </summary>
    public async Task StartAsync(IProgress<string> log, CancellationToken cancellationToken)
    {
        var file = CamerasFile.Load(camerasFile);
        cameras = file;
        var connected = await adb.DevicesAsync(cancellationToken);
        foreach (var camera in file.Cameras.Where(c => !connected.Contains(c.Serial)))
        {
            log.Report($"camera: {camera.Name} ({camera.Serial}) がつながっていません");
        }

        started = file.Cameras.Where(c => connected.Contains(c.Serial)).ToList();
        if (started.Count == 0)
        {
            throw new LauncherException($"{camerasFile} のスマホが 1 台もつながっていません");
        }

        for (var i = 0; i < started.Count; i++)
        {
            var camera = started[i];
            await adb.ShellAsync(camera.Serial, ["am", "force-stop", file.Package], cancellationToken);
            // The phone reaches ball_tracker at its own localhost through adb.
            foreach (var port in new[] { ports.BallTrackerCameraControl, ports.BallTrackerCameraData, ports.BallTrackerCameraVideo })
            {
                var tcp = $"tcp:{port.ToString(CultureInfo.InvariantCulture)}";
                var reverse = await adb.RunAsync(camera.Serial, ["reverse", tcp, tcp], cancellationToken);
                if (!reverse.Succeeded)
                {
                    throw new LauncherException($"{camera.Name}: adb reverse {tcp} が失敗しました: {reverse.Output.Trim()}");
                }
            }

            var result = await BroadcastAsync(file.Package, camera, "SET_CONFIG", ConfigExtras(camera), cancellationToken);
            if (DeviceIdPattern().Match(result) is { Success: true } match)
            {
                if (file.SetDeviceId(camera.Serial, match.Groups[1].Value))
                {
                    log.Report($"camera: {camera.Name} の device_id を {camerasFile} に書きました");
                }
            }
            else
            {
                log.Report($"camera: {camera.Name} が device_id を返しません (アプリが古い?): {result}");
            }

            await adb.ShellAsync(
                camera.Serial,
                ["am", "start", "-n", $"{file.Package}/com.taichi765.struckoutCameraApp.MainActivity", "--es", "screen", "camera"],
                cancellationToken);
            log.Report($"camera: {camera.Name} ({camera.Serial}) を開きました");
            if (i < started.Count - 1)
            {
                await Task.Delay(OpenInterval, cancellationToken);
            }
        }
    }

    public async Task<Health> CheckAsync(CancellationToken cancellationToken)
    {
        if (cameras is null)
        {
            return Health.Down("起動していません");
        }

        var connected = await adb.DevicesAsync(cancellationToken);
        foreach (var camera in started)
        {
            if (!connected.Contains(camera.Serial))
            {
                return Health.Down($"{camera.Name} ({camera.Serial}) が外れました");
            }

            var pid = await adb.ShellAsync(camera.Serial, ["pidof", cameras.Package], cancellationToken);
            if (string.IsNullOrWhiteSpace(pid))
            {
                return Health.Down($"{camera.Name} のアプリが動いていません");
            }
        }

        return Health.Up($"{string.Join("、", started.Select(c => c.Name))} で動いています");
    }

    public async Task StopAsync(IProgress<string> log, CancellationToken cancellationToken)
    {
        if (cameras is null)
        {
            return;
        }

        var connected = await adb.DevicesAsync(cancellationToken);
        foreach (var camera in started.Where(c => connected.Contains(c.Serial)))
        {
            log.Report($"camera: {camera.Name} のアプリを止めます");
            await adb.ShellAsync(camera.Serial, ["am", "force-stop", cameras.Package], cancellationToken);
        }

        cameras = null;
        started = [];
    }

    internal static IReadOnlyList<string> ConfigExtras(CameraEntry camera)
    {
        var extras = new List<string>
        {
            "--es", "output", camera.Output,
            "--es", "focal_ratio", camera.FocalRatio.ToString(CultureInfo.InvariantCulture),
            "--es", "pose", CamerasFile.JoinNumbers(camera.Pose),
        };
        if (camera.Area is { Count: > 0 } area)
        {
            extras.AddRange(["--es", "area", CamerasFile.JoinNumbers(area)]);
        }

        return extras;
    }

    /// <returns>The data of "Broadcast completed: result=0, data=...".</returns>
    private async Task<string> BroadcastAsync(
        string package, CameraEntry camera, string action, IReadOnlyList<string> extras, CancellationToken cancellationToken)
    {
        // -f 32 (FLAG_INCLUDE_STOPPED_PACKAGES): delivered even after force-stop
        var output = await adb.ShellAsync(
            camera.Serial,
            ["am", "broadcast", "-f", "32", "-a", ActionPrefix + action,
                "-n", $"{package}/com.taichi765.struckoutCameraApp.remote.RemoteControlReceiver", .. extras],
            cancellationToken);
        var match = BroadcastDataPattern().Match(output);
        return match.Success ? match.Groups[1].Value : output.Trim();
    }

    [GeneratedRegex("data=\"(.*)\"")]
    private static partial Regex BroadcastDataPattern();

    [GeneratedRegex(@"device_id=(\S+)")]
    private static partial Regex DeviceIdPattern();
}
