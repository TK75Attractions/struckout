using Struckout.Launcher.Core.GitHub;
using Struckout.Launcher.Core.Install;
using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.Core.Tests;

public class PlatformTests
{
    private static readonly Release Latest = new(
        "2026-10-19.0",
        DateTimeOffset.UnixEpoch,
        new[]
        {
            "ball-tracker-arm64-macos", "ball-tracker-x86_64-linux", "ball-tracker-x86_64-windows",
            "projector-arm64-macos.zip", "projector-x86_64-linux.zip", "projector-x86_64-windows.zip",
        }.Select(name => new ReleaseAsset(name, new Uri($"https://example.com/{name}"), 1)).ToList());

    [Theory]
    [InlineData(OperatingSystemKind.Windows, CpuArchitecture.X64, "ball-tracker-x86_64-windows")]
    // Windows on Arm runs the x86_64 build through emulation
    [InlineData(OperatingSystemKind.Windows, CpuArchitecture.Arm64, "ball-tracker-x86_64-windows")]
    [InlineData(OperatingSystemKind.MacOS, CpuArchitecture.Arm64, "ball-tracker-arm64-macos")]
    [InlineData(OperatingSystemKind.Linux, CpuArchitecture.X64, "ball-tracker-x86_64-linux")]
    public void PicksTheAssetThisHostRuns(OperatingSystemKind os, CpuArchitecture architecture, string expected)
    {
        var asset = ReleaseAssets.Pick(Latest, new HostPlatform(os, architecture), "ball-tracker");

        Assert.Equal(expected, asset.Name);
    }

    [Fact]
    public void PicksArchives()
    {
        var asset = ReleaseAssets.Pick(
            Latest, new HostPlatform(OperatingSystemKind.Linux, CpuArchitecture.X64), "projector", ".zip");

        Assert.Equal("projector-x86_64-linux.zip", asset.Name);
    }

    [Theory]
    [InlineData(OperatingSystemKind.Linux, CpuArchitecture.Arm64)]
    [InlineData(OperatingSystemKind.MacOS, CpuArchitecture.X64)]
    public void UnsupportedHostIsAnError(OperatingSystemKind os, CpuArchitecture architecture)
    {
        var error = Assert.Throws<LauncherException>(
            () => ReleaseAssets.Pick(Latest, new HostPlatform(os, architecture), "ball-tracker"));

        Assert.Contains("この PC", error.Message, StringComparison.Ordinal);
    }

    [Fact]
    public void LinuxPrerequisitesAreInstalledByTheUser()
    {
        var linux = new HostPlatform(OperatingSystemKind.Linux, CpuArchitecture.X64);

        Assert.IsType<ManualInstall>(PrerequisiteInstaller.MethodFor(PrerequisiteId.Docker, linux));
        Assert.IsType<ManualInstall>(PrerequisiteInstaller.MethodFor(PrerequisiteId.Ffmpeg, linux));
        var adb = Assert.IsType<PlatformToolsDownload>(PrerequisiteInstaller.MethodFor(PrerequisiteId.Adb, linux));
        Assert.EndsWith("platform-tools-latest-linux.zip", adb.Url.AbsoluteUri, StringComparison.Ordinal);
    }
}
