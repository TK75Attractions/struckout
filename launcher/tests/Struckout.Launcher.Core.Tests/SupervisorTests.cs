using Struckout.Launcher.Core.Install;
using Struckout.Launcher.Core.Launch;
using Struckout.Launcher.Core.Modules;
using Struckout.Launcher.Core.Platform;
using Struckout.Launcher.Core.Spec;

namespace Struckout.Launcher.Core.Tests;

public sealed class SupervisorTests
{
    [Theory]
    [InlineData("device_id=new-left", false, "new-left", "old-right")]
    [InlineData("device_id=new-left", true, "new-left", "new-right")]
    [InlineData("old app", true, "old-left", "old-right")]
    public async Task RecordsDeviceIdsBeforeAttemptingToStartTracker(string response, bool rightConnected, string expectedLeft, string expectedRight)
    {
        var directory = Path.Combine(Path.GetTempPath(), $"supervisor-{Guid.NewGuid()}");
        Directory.CreateDirectory(directory);
        try
        {
            var paths = new AppPaths(directory, directory);
            await File.WriteAllTextAsync(paths.CamerasFile, """
                { "package": "camera.highspeed", "cameras": [
                  { "serial": "left", "device_id": "old-left", "name": "left", "output": "network", "focal_ratio": 1, "pose": [] },
                  { "serial": "right", "device_id": "old-right", "name": "right", "output": "network", "focal_ratio": 1, "pose": [] }
                ] }
                """, TestContext.Current.CancellationToken);
            var config = LauncherConfig.Parse("""
                modules = ["ball-tracker", "camera"]
                machine-id = 1
                """, paths.LauncherConfigFile);
            var commands = new Phones(response, rightConnected);
            var ports = new ServerPorts(1, 2, 3, 4, 5, 0);
            var tracker = new Tracker(() =>
            {
                Assert.Equal([expectedLeft, expectedRight], CamerasFile.Load(paths.CamerasFile).Cameras.Select(c => c.DeviceId));
                Assert.Equal(rightConnected ? 2 : 1, commands.Calls.Count(c => c.Contains("STATUS", StringComparison.Ordinal)));
                Assert.DoesNotContain(commands.Calls, c => c.Contains("am start", StringComparison.Ordinal));
            });
            using var supervisor = new Supervisor(config, new InstallState(), ports,
                [tracker, new CameraRunner(new Adb("adb", commands), paths.CamerasFile, ports)]);
            await supervisor.StartAllAsync(new Progress<string>(), TestContext.Current.CancellationToken);
            Assert.True(tracker.Attempted);
        }
        finally
        {
            Directory.Delete(directory, recursive: true);
        }
    }

    private sealed class Tracker(Action onStart) : IModuleRunner
    {
        public ModuleId Id => ModuleId.BallTracker;
        public bool Attempted { get; private set; }
        public Task StartAsync(IProgress<string> log, CancellationToken cancellationToken)
        {
            Attempted = true;
            onStart();
            throw new LauncherException("テスト: 起動を中断します");
        }
        public Task<Health> CheckAsync(CancellationToken cancellationToken) => Task.FromResult(Health.Down("テスト"));
        public Task StopAsync(IProgress<string> log, CancellationToken cancellationToken) => Task.CompletedTask;
    }

    private sealed class Phones(string response, bool rightConnected) : ICommandRunner
    {
        public List<string> Calls { get; } = [];
        public Task<CommandResult> RunAsync(string fileName, IReadOnlyList<string> arguments, CommandOptions? options = null, CancellationToken cancellationToken = default)
        {
            var command = string.Join(' ', arguments);
            Calls.Add(command);
            var data = arguments.Contains("right") ? response.Replace("left", "right", StringComparison.Ordinal) : response;
            return Task.FromResult(new CommandResult(0, command == "devices"
                ? "left\tdevice\n" + (rightConnected ? "right\tdevice\n" : "")
                : $"Broadcast completed: result=0, data=\"{data}\""));
        }
    }
}
