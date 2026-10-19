using Struckout.Launcher.Core.Install;
using Struckout.Launcher.Core.Modules;

namespace Struckout.Launcher.Core.Tests;

public class InstallStateTests
{
    private static readonly DateTimeOffset Published = new(2026, 10, 18, 6, 35, 8, TimeSpan.Zero);

    [Fact]
    public void RoundTrips()
    {
        var state = new InstallState();
        state.Modules[ModuleId.BallTracker] = new ExecutableInstallation
        {
            ReleaseTag = "2026-10-19.0",
            ReleasePublishedAt = Published,
            Directory = "/data/modules/ball-tracker",
            Executable = "/data/modules/ball-tracker/ball_tracker",
        };
        state.Modules[ModuleId.Camera] = new ApkInstallation
        {
            ReleaseTag = "2026-10-19.0",
            ReleasePublishedAt = Published,
            Directory = "/data/modules/camera",
            Package = "com.taichi765.struckoutCameraApp.highspeed",
            Devices = ["166810122536"],
        };
        state.Prerequisites[PrerequisiteId.Docker] = new PackageManagerPrerequisite(PackageManager.Winget, "Docker.DockerDesktop");
        state.Prerequisites[PrerequisiteId.Adb] = new FoundPrerequisite("/usr/bin/adb");

        var parsed = InstallState.Parse(state.Serialize());

        Assert.Equal(state.Modules[ModuleId.BallTracker], parsed.Modules[ModuleId.BallTracker]);
        var camera = Assert.IsType<ApkInstallation>(parsed.Modules[ModuleId.Camera]);
        Assert.Equal(["166810122536"], camera.Devices);
        Assert.Equal(state.Prerequisites, parsed.Prerequisites);
    }

    [Fact]
    public void UsesTheSlugsOfTheSettings()
    {
        var state = new InstallState();
        state.Prerequisites[PrerequisiteId.Ffmpeg] = new PackageManagerPrerequisite(PackageManager.HomebrewCask, "ffmpeg");
        state.Modules[ModuleId.GameMaster] = new ComposeInstallation
        {
            ReleaseTag = "2026-10-19.0",
            ReleasePublishedAt = Published,
            Directory = "/data/modules/game-master",
        };

        var json = state.Serialize();

        Assert.Contains("\"game-master\": {", json, StringComparison.Ordinal);
        Assert.Contains("\"kind\": \"compose\"", json, StringComparison.Ordinal);
        Assert.Contains("\"manager\": \"homebrew-cask\"", json, StringComparison.Ordinal);
        Assert.Contains("\"release-published-at\"", json, StringComparison.Ordinal);
    }

    [Fact]
    public void MissingFileIsEmpty()
    {
        var state = InstallState.Load(Path.Combine(Path.GetTempPath(), Guid.NewGuid().ToString(), "install-state.json"));

        Assert.Empty(state.Modules);
        Assert.Empty(state.Prerequisites);
    }
}
