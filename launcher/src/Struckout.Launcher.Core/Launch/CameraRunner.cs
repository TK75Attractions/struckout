using System.Globalization;
using System.Text.RegularExpressions;
using Grpc.Core;
using Grpc.Net.Client;
using Struckout.Launcher.Core.Modules;
using Struckout.Launcher.Core.Spec;
using Tk75Attractions.Struckout.BallTracker.V1;

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
    private readonly HashSet<string> opened = new(StringComparer.Ordinal);
    private IProgress<string>? launchLog;
    private DateTimeOffset? lastOpenedAt;

    public ModuleId Id => ModuleId.Camera;

    /// <summary>
    /// Records the connected phones' IDs before ball_tracker starts: it reads cameras.json only once,
    /// so an ID changed by reinstalling the app must be saved before it assigns camera IDs and calibration.
    /// STATUS only queries the app; capture and port forwarding remain part of StartAsync.
    /// </summary>
    public async Task RecordDeviceIdsAsync(IProgress<string> log, CancellationToken cancellationToken)
    {
        var file = CamerasFile.Load(camerasFile);
        var connected = await adb.DevicesAsync(cancellationToken);
        foreach (var camera in file.Cameras)
        {
            if (!connected.Contains(camera.Serial))
            {
                log.Report($"camera: {camera.Name} ({camera.Serial}) がつながっていません");
                continue;
            }

            var result = await BroadcastAsync(file.Package, camera, "STATUS", [], cancellationToken);
            RecordDeviceId(file, camera, result, log);
        }
    }

    /// <summary>
    /// Forwards ball_tracker's ports to each phone, sends its settings, records the ID it reports and opens the app.
    /// </summary>
    public async Task StartAsync(IProgress<string> log, CancellationToken cancellationToken)
    {
        var file = CamerasFile.Load(camerasFile);
        cameras = file;
        opened.Clear();
        launchLog = log;
        lastOpenedAt = null;
        var connected = await adb.DevicesAsync(cancellationToken);
        foreach (var camera in file.Cameras.Where(c => !connected.Contains(c.Serial)))
        {
            log.Report($"camera: {camera.Name} ({camera.Serial}) がつながっていません");
        }

        var started = file.Cameras.Where(c => connected.Contains(c.Serial)).ToList();
        if (started.Count == 0)
        {
            throw new LauncherException($"{camerasFile} のスマホが 1 台もつながっていません");
        }

        foreach (var camera in started)
        {
            await OpenAsync(file, camera, log, cancellationToken);
        }
    }

    public async Task<Health> CheckAsync(CancellationToken cancellationToken)
    {
        if (cameras is null)
        {
            return Health.Down("起動していません");
        }

        var connected = await adb.DevicesAsync(cancellationToken);
        foreach (var camera in cameras.Cameras.Where(c => connected.Contains(c.Serial) && !opened.Contains(c.Serial)))
        {
            try
            {
                await OpenAsync(cameras, camera, launchLog!, cancellationToken);
            }
            catch (LauncherException error)
            {
                return Health.Down(error.Message);
            }
        }

        List<CameraStatus>? statuses = null;
        try
        {
            using var channel = GrpcChannel.ForAddress($"http://127.0.0.1:{ports.BallTrackerHealth}");
            var client = new CameraStatusService.CameraStatusServiceClient(channel);
            using var call = client.ListCameraStatusAsync(new ListCameraStatusRequest(),
                deadline: DateTime.UtcNow.AddSeconds(3), cancellationToken: cancellationToken);
            var response = await call.ResponseAsync;
            statuses = response.Cameras.ToList();
        }
        catch (RpcException)
        {
            cancellationToken.ThrowIfCancellationRequested();
            // Do not reuse stale status or restart the app when the probe fails.
        }

        var now = DateTimeOffset.UtcNow;
        var observations = new List<CameraObservation>();
        for (var i = 0; i < cameras.Cameras.Count; i++)
        {
            var camera = cameras.Cameras[i];
            var isConnected = connected.Contains(camera.Serial);
            var pid = isConnected ? await adb.RunAsync(camera.Serial, ["shell", "pidof", cameras.Package], cancellationToken) : null;
            var pids = pid?.Output.Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries) ?? [];
            // IDs follow cameras.json order; device IDs prevent accepting another phone's frames.
            var status = statuses?.FirstOrDefault(s => s.CameraId == (uint)i
                && (string.IsNullOrEmpty(camera.DeviceId) || s.DeviceId == camera.DeviceId));
            // This runs every poll while the phone captures at high speed, and dumpsys window is heavy,
            // so the screen is only inspected to explain why frames are not arriving.
            var screen = isConnected && !CameraReadiness.IsReceiving(status, now)
                ? await adb.ScreenStateAsync(camera.Serial, cancellationToken)
                : ScreenState.Unknown;
            observations.Add(new(camera, isConnected, screen, pid is { Succeeded: true } && pids.Length > 0
                && pids.All(part => uint.TryParse(part, out var value) && value > 0), status));
        }

        return CameraReadiness.Evaluate(observations, now, statuses is not null);
    }

    private async Task OpenAsync(CamerasFile file, CameraEntry camera, IProgress<string> log, CancellationToken cancellationToken)
    {
        // Preserve the capture startup interval for late USB connections as well.
        if (lastOpenedAt is { } last)
        {
            var remaining = OpenInterval - (DateTimeOffset.UtcNow - last);
            if (remaining > TimeSpan.Zero)
            {
                await Task.Delay(remaining, cancellationToken);
            }
        }

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
        RecordDeviceId(file, camera, result, log);

        await adb.ShellAsync(camera.Serial, ["input", "keyevent", "KEYCODE_WAKEUP"], cancellationToken);
        await adb.ShellAsync(
            camera.Serial,
            ["am", "start", "-n", $"{file.Package}/com.taichi765.struckoutCameraApp.MainActivity", "--es", "screen", "camera"],
            cancellationToken);
        log.Report($"camera: {camera.Name} ({camera.Serial}) を開きました");
        opened.Add(camera.Serial);
        lastOpenedAt = DateTimeOffset.UtcNow;
    }

    private void RecordDeviceId(CamerasFile file, CameraEntry camera, string result, IProgress<string> log)
    {
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
    }

    public async Task StopAsync(IProgress<string> log, CancellationToken cancellationToken)
    {
        if (cameras is null)
        {
            return;
        }

        var connected = await adb.DevicesAsync(cancellationToken);
        foreach (var camera in cameras.Cameras.Where(c => opened.Contains(c.Serial) && connected.Contains(c.Serial)))
        {
            log.Report($"camera: {camera.Name} のアプリを止めます");
            await adb.ShellAsync(camera.Serial, ["am", "force-stop", cameras.Package], cancellationToken);
        }

        cameras = null;
        opened.Clear();
        launchLog = null;
        lastOpenedAt = null;
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
