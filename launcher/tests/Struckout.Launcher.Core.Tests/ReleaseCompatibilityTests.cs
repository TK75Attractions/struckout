using Struckout.Launcher.Core.Install;
using Struckout.Launcher.Core.Modules;

namespace Struckout.Launcher.Core.Tests;

public class ReleaseCompatibilityTests
{
    [Fact]
    public void MatchingReleasesAreCompatible()
    {
        var state = State(
            (ModuleId.GameMaster, "2026-10-19.0"),
            (ModuleId.Projector, "2026-10-19.0"));

        Assert.Null(ReleaseCompatibility.FindMismatch([ModuleId.GameMaster, ModuleId.Projector], state));
    }

    [Fact]
    public void DifferentReleasesReturnAnActionableMessage()
    {
        var state = State(
            (ModuleId.GameMaster, "2026-10-19.0"),
            (ModuleId.Projector, "2026-10-28.1"));

        var message = ReleaseCompatibility.FindMismatch([ModuleId.GameMaster, ModuleId.Projector], state);

        Assert.NotNull(message);
        Assert.Contains("game-master は 2026-10-19.0", message, StringComparison.Ordinal);
        Assert.Contains("projector は 2026-10-28.1", message, StringComparison.Ordinal);
        Assert.Contains("「更新」", message, StringComparison.Ordinal);
    }

    [Fact]
    public void ModulesWithoutInstallRecordsAreIgnored()
    {
        var state = State((ModuleId.GameMaster, "2026-10-19.0"));

        Assert.Null(ReleaseCompatibility.FindMismatch([ModuleId.GameMaster, ModuleId.Projector], state));
    }

    [Fact]
    public void CameraReleaseTagIsIgnored()
    {
        var state = State(
            (ModuleId.GameMaster, "2026-10-19.0"),
            (ModuleId.Camera, "2026-10-28.1"));

        Assert.Null(ReleaseCompatibility.FindMismatch([ModuleId.GameMaster, ModuleId.Camera], state));
    }

    private static InstallState State(params (ModuleId Id, string Tag)[] installations)
    {
        var state = new InstallState();
        foreach (var (id, tag) in installations)
        {
            state.Modules[id] = new ExecutableInstallation
            {
                ReleaseTag = tag,
                ReleasePublishedAt = DateTimeOffset.UnixEpoch,
                Directory = $"/modules/{id.Slug()}",
                Executable = $"/modules/{id.Slug()}/app",
            };
        }

        return state;
    }
}
