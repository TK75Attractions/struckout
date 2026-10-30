using System.Net;
using Struckout.Launcher.Core.GitHub;
using Struckout.Launcher.Core.Install;
using Struckout.Launcher.Core.Launch;
using Struckout.Launcher.Core.Modules;
using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.Core.Tests;

public sealed class CameraInstallerTests
{
    [Fact]
    public async Task GrantsRuntimePermissionsWhenInstalling()
    {
        var commands = new FakeAdb(0, "Success") { InstallSucceeds = true };
        Assert.False(await CameraInstaller.InstallOnDeviceAsync(new Adb("adb", commands), "serial", "camera.apk",
            "camera.highspeed", new Messages(), TestContext.Current.CancellationToken));
        Assert.Equal(["-s serial install -r -g camera.apk"], commands.Calls);
    }

    [Fact]
    public async Task RecordsExistingAppForInitialLauncherConfiguration()
    {
        var directory = Path.Combine(Path.GetTempPath(), $"camera-install-{Guid.NewGuid()}");
        Directory.CreateDirectory(directory);
        try
        {
            var paths = new AppPaths(directory, directory);
            await File.WriteAllTextAsync(paths.CamerasFile, """
                { "package": "camera.highspeed", "cameras": [
                  { "serial": "serial", "name": "left", "output": "network", "focal_ratio": 1, "pose": [] }
                ] }
                """, TestContext.Current.CancellationToken);
            var commands = new FakeAdb(0, "package:/data/app/camera/base.apk");
            using var http = new HttpClient(new ApkResponse());
            var installer = new CameraInstaller(paths, new HttpDownloader(http), commands);
            var release = new Release("test", DateTimeOffset.UtcNow,
                [new ReleaseAsset("camera.apk", new Uri("https://example.com/camera.apk"), 5,
                    Sha256Digest.ParseGitHub("sha256:2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"))]);
            var result = await installer.InstallAsync(new InstallContext(release,
                new Dictionary<PrerequisiteId, string> { [PrerequisiteId.Adb] = "adb" }, new Messages(), null,
                TestContext.Current.CancellationToken), null);
            var camera = Assert.IsType<ApkInstallation>(result);
            Assert.Equal(["serial"], camera.Devices);
            Assert.Equal(["serial"], camera.ExistingDevices);
            var state = new InstallState();
            state.Modules[ModuleId.Camera] = camera;
            state.Save(paths.InstallStateFile);
            Assert.True(LauncherConfigTemplate.WriteIfMissing(paths.LauncherConfigFile, state.Modules.Keys));
            Assert.Contains("\"camera\"", await File.ReadAllTextAsync(paths.LauncherConfigFile, TestContext.Current.CancellationToken), StringComparison.Ordinal);
            Assert.DoesNotContain(commands.Calls, c => c.Contains("uninstall", StringComparison.Ordinal));
        }
        finally
        {
            Directory.Delete(directory, recursive: true);
        }
    }

    [Theory]
    [InlineData(0, "package:/data/app/camera/base.apk\n", true)]
    [InlineData(0, "", false)]
    [InlineData(1, "package:/data/app/camera/base.apk", false)]
    [InlineData(0, "Error: package missing", false)]
    public async Task KeepsOnlyAnInstalledMatchingPackage(int pathExitCode, string pathOutput, bool canKeep)
    {
        var commands = new FakeAdb(pathExitCode, pathOutput);
        var log = new Messages();
        var install = CameraInstaller.InstallOnDeviceAsync(new Adb("adb", commands), "serial", "camera.apk", "camera.highspeed", log, TestContext.Current.CancellationToken);
        if (canKeep)
        {
            Assert.True(await install);
            Assert.Contains(log.Values, text => text.Contains("署名が違う既存のアプリを使います。リリースの版には更新されません", StringComparison.Ordinal));
        }
        else
        {
            var error = await Assert.ThrowsAsync<LauncherException>(() => install);
            Assert.Contains("INSTALL_FAILED_UPDATE_INCOMPATIBLE", error.Message, StringComparison.Ordinal);
        }

        Assert.Equal(["-s serial install -r -g camera.apk", "-s serial shell pm path camera.highspeed"], commands.Calls);
    }

    [Fact]
    public void ReadsOldStateAndRoundTripsExistingDevices()
    {
        const string old = """
            { "modules": { "camera": { "kind": "apk", "release-tag": "test", "release-published-at": "2026-10-30T00:00:00Z",
              "directory": "camera", "package": "camera.highspeed", "devices": ["serial"] } } }
            """;
        var state = InstallState.Parse(old);
        var camera = Assert.IsType<ApkInstallation>(state.Modules[ModuleId.Camera]);
        Assert.Empty(camera.ExistingDevices);
        state.Modules[ModuleId.Camera] = camera with { ExistingDevices = ["serial"] };
        var reloaded = Assert.IsType<ApkInstallation>(InstallState.Parse(state.Serialize()).Modules[ModuleId.Camera]);
        Assert.Equal(["serial"], reloaded.Devices);
        Assert.Equal(["serial"], reloaded.ExistingDevices);
    }

    private sealed class Messages : IProgress<string>
    {
        public List<string> Values { get; } = [];
        public void Report(string value) => Values.Add(value);
    }

    private sealed class FakeAdb(int pathExitCode, string pathOutput) : ICommandRunner
    {
        public bool InstallSucceeds { get; init; }
        public List<string> Calls { get; } = [];
        public Task<CommandResult> RunAsync(string fileName, IReadOnlyList<string> arguments, CommandOptions? options = null, CancellationToken cancellationToken = default)
        {
            var command = string.Join(' ', arguments);
            Calls.Add(command);
            if (command == "devices")
                return Task.FromResult(new CommandResult(0, "serial\tdevice\n"));
            return Task.FromResult(command.Contains("install -r", StringComparison.Ordinal)
                ? InstallSucceeds ? new CommandResult(0, "Success") : new CommandResult(1, "Failure [INSTALL_FAILED_UPDATE_INCOMPATIBLE]")
                : new CommandResult(pathExitCode, pathOutput));
        }
    }

    private sealed class ApkResponse : HttpMessageHandler
    {
        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken) =>
            Task.FromResult(new HttpResponseMessage(HttpStatusCode.OK) { Content = new StringContent("hello") });
    }
}
