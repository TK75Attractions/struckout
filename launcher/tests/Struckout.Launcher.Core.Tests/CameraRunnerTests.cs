using Struckout.Launcher.Core.Launch;
using Struckout.Launcher.Core.Modules;
using Struckout.Launcher.Core.Platform;
using Struckout.Launcher.Core.Spec;

namespace Struckout.Launcher.Core.Tests;

public sealed class CameraRunnerTests
{
    [Fact]
    public async Task OpensLateCameraOnceAndDoesNotRestartStoppedApps()
    {
        var path = Path.Combine(Path.GetTempPath(), $"camera-runner-{Guid.NewGuid()}.json");
        await File.WriteAllTextAsync(path, """
            { "package": "camera.highspeed", "cameras": [
              { "serial": "left", "name": "left", "output": "network", "focal_ratio": 1, "pose": [] },
              { "serial": "right", "name": "right", "output": "network", "focal_ratio": 1, "pose": [] }
            ] }
            """, TestContext.Current.CancellationToken);
        try
        {
            var commands = new Phones();
            var runner = new CameraRunner(new Adb("adb", commands), path, new ServerPorts(1, 2, 3, 4, 5, 0));
            await runner.StartAsync(new Progress<string>(), TestContext.Current.CancellationToken);
            Assert.Single(commands.Calls, c => c.Contains("am start", StringComparison.Ordinal));
            commands.RightConnected = true;
            Assert.Equal(HealthKind.Down, (await runner.CheckAsync(TestContext.Current.CancellationToken)).Kind);
            Assert.Equal(HealthKind.Down, (await runner.CheckAsync(TestContext.Current.CancellationToken)).Kind);
            Assert.Equal(2, commands.Calls.Count(c => c.Contains("am start", StringComparison.Ordinal)));
            Assert.Equal(2, commands.Calls.Count(c => c.Contains("am force-stop", StringComparison.Ordinal)));
            foreach (var serial in new[] { "left", "right" })
            {
                var calls = commands.Calls.Where(c => c.StartsWith($"-s {serial} ", StringComparison.Ordinal)).ToList();
                Assert.Contains(calls, c => c.Contains("reverse tcp:3 tcp:3", StringComparison.Ordinal));
                Assert.Contains(calls, c => c.Contains("SET_CONFIG", StringComparison.Ordinal));
                Assert.True(calls.FindIndex(c => c.Contains("KEYCODE_WAKEUP", StringComparison.Ordinal))
                    < calls.FindIndex(c => c.Contains("am start", StringComparison.Ordinal)));
            }

            await runner.StopAsync(new Progress<string>(), TestContext.Current.CancellationToken);
            Assert.Equal(4, commands.Calls.Count(c => c.Contains("am force-stop", StringComparison.Ordinal)));
        }
        finally
        {
            File.Delete(path);
        }
    }

    private sealed class Phones : ICommandRunner
    {
        public bool RightConnected { get; set; }
        public List<string> Calls { get; } = [];
        public Task<CommandResult> RunAsync(string fileName, IReadOnlyList<string> arguments, CommandOptions? options = null, CancellationToken cancellationToken = default)
        {
            var command = string.Join(' ', arguments);
            Calls.Add(command);
            var output = command switch
            {
                "devices" => "left\tdevice\n" + (RightConnected ? "right\tdevice\n" : ""),
                _ when command.Contains("dumpsys power", StringComparison.Ordinal) => "mWakefulness=Awake",
                _ when command.Contains("dumpsys window", StringComparison.Ordinal) => "mShowingLockscreen=false",
                _ => "",
            };
            return Task.FromResult(new CommandResult(0, output));
        }
    }
}
