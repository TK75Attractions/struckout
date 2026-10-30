using Struckout.Launcher.Core.Launch;
using Struckout.Launcher.Core.Modules;
using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.Core.Tests;

public class GameMasterComposeTests
{
    [Fact]
    public async Task MountsEditableConfigAndRecreatesAppOnStart()
    {
        var root = Path.Combine(Path.GetTempPath(), "struckout-launcher-test-" + Guid.NewGuid());
        try
        {
            var paths = new AppPaths(Path.Combine(root, "config", "struckout", "launcher"), Path.Combine(root, "data"));
            var directory = Path.Combine(root, "module");
            Directory.CreateDirectory(directory);
            var runner = new RecordingRunner();
            var compose = new GameMasterCompose(HostPlatform.Current, "docker", directory, "test", "password", paths.GameMasterConfigFile, runner);
            var gameMaster = new GameMasterRunner(compose, 8020);

            await gameMaster.StartAsync(new Progress<string>(), CancellationToken.None);

            Assert.Equal("game_duration_secs = 90\n", File.ReadAllText(paths.GameMasterConfigFile));
            var mount = File.ReadAllText(Path.Combine(directory, "compose.launcher.yaml"));
            Assert.Contains("source: ${GAME_MASTER_CONFIG_PATH:", mount, StringComparison.Ordinal);
            Assert.Contains("target: /etc/game_master/config.toml", mount, StringComparison.Ordinal);
            Assert.Contains("create_host_path: false", mount, StringComparison.Ordinal);
            Assert.Equal(Path.GetFullPath(paths.GameMasterConfigFile), runner.Commands[1].Options!.Environment["GAME_MASTER_CONFIG_PATH"]);
            Assert.Equal(["up", "--detach", "--wait", "db"], runner.Commands[1].Arguments.TakeLast(4));
            Assert.Equal(["up", "--detach", "--force-recreate", "--no-deps", "app"], runner.Commands[2].Arguments.TakeLast(5));

            File.WriteAllText(paths.GameMasterConfigFile, "game_duration_secs = 120\n");
            await gameMaster.StartAsync(new Progress<string>(), CancellationToken.None);
            Assert.Equal("game_duration_secs = 120\n", File.ReadAllText(paths.GameMasterConfigFile));
        }
        finally
        {
            Directory.Delete(root, recursive: true);
        }
    }

    private sealed class RecordingRunner : ICommandRunner
    {
        public List<(IReadOnlyList<string> Arguments, CommandOptions? Options)> Commands { get; } = [];

        public Task<CommandResult> RunAsync(
            string fileName, IReadOnlyList<string> arguments, CommandOptions? options = null,
            CancellationToken cancellationToken = default)
        {
            Commands.Add((arguments, options));
            return Task.FromResult(new CommandResult(0, ""));
        }
    }
}
