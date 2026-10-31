using Struckout.Launcher.Core.Launch;
using Struckout.Launcher.Core.Modules;

namespace Struckout.Launcher.Core.Tests;

public class LauncherConfigTests
{
    private static readonly string ConfigPath = Path.Combine(Path.GetTempPath(), "struckout", "launcher.toml");

    [Fact]
    public void TemplateIsValid()
    {
        var text = LauncherConfigTemplate.Render([ModuleId.Camera, ModuleId.BallTracker]);

        var config = LauncherConfig.Parse(text, ConfigPath);

        Assert.Equal(1u, config.MachineId);
        Assert.Equal(new HashSet<ModuleId> { ModuleId.BallTracker, ModuleId.Camera }, config.Modules);
        Assert.Equal(CollisionOutput.Network, config.BallTracker.Output);
        Assert.Null(config.BallTracker.CsvPath);
        Assert.Empty(config.BallTracker.ExtraArguments);
        Assert.Equal(new ProjectorSettings(2, true, null, false), config.Projector);
    }

    [Fact]
    public void LocalModulesAreReachedAtLocalhost()
    {
        const string text = """
            modules = ["game-master", "projector"]
            [hosts]
            ball-tracker = "pc1.local"
            game-master = "pc3.local"
            """;

        var config = LauncherConfig.Parse(text, ConfigPath);

        Assert.Equal("pc1.local", config.HostOf(ModuleId.BallTracker));
        Assert.Equal("127.0.0.1", config.HostOf(ModuleId.GameMaster));
    }

    [Fact]
    public void ProjectorSimpleModeCanBeEnabled()
    {
        var config = LauncherConfig.Parse("[projector]\nsimple-mode = true\n", ConfigPath);

        Assert.True(config.Projector.SimpleMode);
    }

    [Fact]
    public void RelativePathsAreRelativeToTheConfig()
    {
        const string text = """
            [ball-tracker]
            output = "csv"
            csv-path = "out/collisions.csv"
            extra-args = ["--time-trim-ms", "3"]
            """;

        var config = LauncherConfig.Parse(text, ConfigPath);

        Assert.Equal(CollisionOutput.Csv, config.BallTracker.Output);
        Assert.Equal(
            Path.Combine(Path.GetDirectoryName(ConfigPath)!, "out", "collisions.csv"),
            config.BallTracker.CsvPath);
        Assert.Equal(["--time-trim-ms", "3"], config.BallTracker.ExtraArguments);
    }

    [Theory]
    [InlineData("modules = [\"ball_tracker\"]", "ball_tracker というモジュールはありません")]
    [InlineData("modules = [\"camera\"]", "ball-tracker も入れてください")]
    [InlineData("machine-id = -1", "machine-id は 0 以上")]
    [InlineData("[ball-tracker]\noutput = \"file\"", "\"network\" か \"csv\"")]
    [InlineData("[ball-tracker]\noutput = \"csv\"", "csv-path も書いてください")]
    [InlineData("[projector]\nmonitor = 0", "projector.monitor は 1 以上")]
    [InlineData("machine_id = 1", "machine_id という設定はありません")]
    [InlineData("[projector]\nscreen = 2", "projector.screen という設定はありません")]
    [InlineData("modules = [", "を読めません")]
    public void WrongSettingsAreReported(string text, string message)
    {
        var error = Assert.Throws<LauncherException>(() => LauncherConfig.Parse(text, ConfigPath));

        Assert.Contains(message, error.Message, StringComparison.Ordinal);
    }

    [Fact]
    public void MissingConfigAsksToInstall()
    {
        var path = Path.Combine(Path.GetTempPath(), Guid.NewGuid().ToString(), "launcher.toml");

        var error = Assert.Throws<LauncherException>(() => LauncherConfig.Load(path));

        Assert.Contains("インストールしてください", error.Message, StringComparison.Ordinal);
    }

    [Fact]
    public void AllErrorsAreReportedAtOnce()
    {
        const string text = """
            modules = ["camera"]
            [projector]
            monitor = 0
            """;

        var error = Assert.Throws<LauncherException>(() => LauncherConfig.Parse(text, ConfigPath));

        Assert.Contains("ball-tracker も入れてください", error.Message, StringComparison.Ordinal);
        Assert.Contains("projector.monitor", error.Message, StringComparison.Ordinal);
    }
}
