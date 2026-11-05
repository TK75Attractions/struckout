using Struckout.Launcher.Core.Launch;
using Struckout.Launcher.Core.Modules;
using Struckout.Launcher.Core.Platform;
using Struckout.Launcher.Core.Spec;

namespace Struckout.Launcher.Core.Tests;

public sealed class CameraSerialsTests : IDisposable
{
    private const string Cameras = """
        {
          "package": "camera.highspeed",
          "cameras": [
            { "serial": "old-left", "device_id": "aaaa", "name": "left", "output": "network", "focal_ratio": 0.97,
              "pose": [0.11, 0.12, 1.23, 136.962, -0.453, -32.331] },
            { "serial": "old-right", "device_id": "bbbb", "name": "right", "output": "network", "focal_ratio": 0.97,
              "pose": [0.11, 1.06, 1.23, 135.14, -1.542, -148.297], "area": [0, 0, 0.5, 1] }
          ]
        }
        """;

    private readonly string path = Path.Combine(Path.GetTempPath(), $"camera-serials-{Guid.NewGuid()}.json");
    private readonly ListLog log = new();

    public CameraSerialsTests() => File.WriteAllText(path, Cameras);

    public void Dispose() => File.Delete(path);

    [Fact]
    public void LeavesConnectedCamerasAlone()
    {
        var plan = CameraSerials.Plan(CamerasFile.Load(path).Cameras, ["old-left", "old-right"]);

        Assert.Empty(plan.Automatic);
        Assert.Empty(plan.UnknownDevices);
        Assert.False(plan.NeedsChoice);
    }

    [Fact]
    public void GivesAReplacedPhoneTheMissingCamera()
    {
        var plan = CameraSerials.Plan(CamerasFile.Load(path).Cameras, ["old-left", "new"]);

        Assert.Equal([new SerialAssignment(1, "right", "new")], plan.Automatic);
        Assert.False(plan.NeedsChoice);
    }

    [Theory]
    [InlineData("new-1,new-2")]
    [InlineData("new-1")]
    [InlineData("old-left,new-1,new-2")]
    public void AsksWhenThePhonesCanBeEitherCamera(string connected)
    {
        var plan = CameraSerials.Plan(CamerasFile.Load(path).Cameras, connected.Split(','));

        Assert.Empty(plan.Automatic);
        Assert.True(plan.NeedsChoice);
    }

    [Fact]
    public void IgnoresAnExtraPhoneWhenEveryCameraIsConnected()
    {
        var plan = CameraSerials.Plan(CamerasFile.Load(path).Cameras, ["old-left", "old-right", "spare"]);

        Assert.Empty(plan.Automatic);
        Assert.Equal(["spare"], plan.UnknownDevices);
        Assert.False(plan.NeedsChoice);
    }

    [Fact]
    public void WritesTheNewSerialAndForgetsThePreviousPhonesId()
    {
        var file = CamerasFile.Load(path);

        Assert.True(CameraSerials.AssignAutomatically(file, ["new", "old-right"], log));

        var reloaded = CamerasFile.Load(path);
        Assert.Equal(["new", "old-right"], reloaded.Cameras.Select(c => c.Serial));
        Assert.Null(reloaded.Cameras[0].DeviceId);
        Assert.Equal("bbbb", reloaded.Cameras[1].DeviceId);
        Assert.Equal([0.11, 0.12, 1.23, 136.962, -0.453, -32.331], reloaded.Cameras[0].Pose);
        Assert.Equal(
            reloaded.Cameras.Select(c => (c.Serial, c.DeviceId)), file.Cameras.Select(c => (c.Serial, c.DeviceId)));
        Assert.Contains(log.Lines, line => line.Contains("old-left から new", StringComparison.Ordinal));
    }

    [Fact]
    public void ReportsPhonesThatNeedAChoiceWithoutWriting()
    {
        var before = File.ReadAllText(path);

        Assert.False(CameraSerials.AssignAutomatically(CamerasFile.Load(path), ["new-1", "new-2"], log));

        Assert.Equal(before, File.ReadAllText(path));
        Assert.Contains("カメラの割り当て", Assert.Single(log.Lines), StringComparison.Ordinal);
    }

    [Fact]
    public void RefusesAPhoneThatIsAlreadyAnotherCamera()
    {
        var file = CamerasFile.Load(path);

        Assert.Throws<LauncherException>(() => file.AssignSerial(0, "old-right"));
        Assert.False(file.AssignSerial(0, "old-left"));
    }

    [Fact]
    public async Task StartsAReplacedPhoneAsItsCamera()
    {
        var phones = new Phones("old-left", "new");
        var runner = new CameraRunner(new Adb("adb", phones), path, new ServerPorts(1, 2, 3, 4, 5, 0));

        await runner.StartAsync(log, TestContext.Current.CancellationToken);

        Assert.Equal("new", CamerasFile.Load(path).Cameras[1].Serial);
        Assert.Contains(phones.Calls, c => c.StartsWith("-s new ", StringComparison.Ordinal) && c.Contains("SET_CONFIG", StringComparison.Ordinal));
        Assert.Contains(phones.Calls, c => c.StartsWith("-s new ", StringComparison.Ordinal) && c.Contains("am start", StringComparison.Ordinal));
    }

    private sealed class Phones(params string[] serials) : ICommandRunner
    {
        public List<string> Calls { get; } = [];

        public Task<CommandResult> RunAsync(
            string fileName, IReadOnlyList<string> arguments, CommandOptions? options = null,
            CancellationToken cancellationToken = default)
        {
            var command = string.Join(' ', arguments);
            Calls.Add(command);
            var output = command == "devices" ? string.Concat(serials.Select(s => $"{s}\tdevice\n")) : "";
            return Task.FromResult(new CommandResult(0, output));
        }
    }
}
