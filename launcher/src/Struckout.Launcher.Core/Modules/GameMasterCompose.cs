using System.Security.Cryptography;
using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.Core.Modules;

/// <summary>Runs <c>docker compose</c> for the game_master that the installer set up.</summary>
/// <remarks>
/// The compose files are the ones in <c>game_master/docker/</c> of the release. <c>compose.prod.yaml</c> replaces the
/// build with the image published to ghcr.io, so the PC doesn't need the source.
/// </remarks>
/// <param name="version">The release tag, which is also the tag of the image.</param>
public sealed class GameMasterCompose(
    HostPlatform platform, string docker, string directory, string version, string password, ICommandRunner runner)
{
    /// <summary>The files under <c>game_master/docker/</c> of the repository, in the order compose merges them.</summary>
    public static IReadOnlyList<string> Files { get; } = ["compose.yaml", "compose.prod.yaml"];

    /// <summary>
    /// Separate from the projects of <c>mise run game-master:*</c> so that a development PC keeps its database.
    /// </summary>
    public const string ProjectName = "struckout-launcher";

    public string Directory { get; } = directory;

    public Task<CommandResult> RunAsync(IReadOnlyList<string> arguments, CancellationToken cancellationToken)
    {
        var all = new List<string> { "compose", "--project-name", ProjectName };
        foreach (var file in Files)
        {
            all.Add("--file");
            all.Add(Path.Combine(Directory, file));
        }

        all.AddRange(arguments);
        return runner.RunAsync(
            docker,
            all,
            new CommandOptions
            {
                WorkingDirectory = Directory,
                Environment = new Dictionary<string, string>
                {
                    ["GAME_MASTER_VERSION"] = version,
                    ["MYSQL_ROOT_PASSWORD"] = password,
                },
            },
            cancellationToken);
    }

    /// <summary>Starts Docker Desktop on Windows and macOS if the engine isn't running.</summary>
    /// <exception cref="LauncherException">The engine doesn't come up.</exception>
    public async Task EnsureEngineRunningAsync(IProgress<string> log, CancellationToken cancellationToken)
    {
        if ((await runner.RunAsync(docker, ["info"], cancellationToken: cancellationToken)).Succeeded)
        {
            return;
        }

        switch (platform.Os)
        {
            case OperatingSystemKind.Windows:
                var desktop = Path.Combine(
                    Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles),
                    "Docker", "Docker", "Docker Desktop.exe");
                if (!File.Exists(desktop))
                {
                    throw new LauncherException($"Docker が動いていません。{desktop} もありません");
                }

                log.Report("Docker Desktop を起動します");
                System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo(desktop) { UseShellExecute = true })
                    ?.Dispose();
                break;
            case OperatingSystemKind.MacOS:
                log.Report("Docker Desktop を起動します");
                await runner.RunAsync("open", ["-a", "Docker"], cancellationToken: cancellationToken);
                break;
            default:
                throw new LauncherException(
                    "Docker が動いていません。sudo systemctl start docker で起動してください");
        }

        var deadline = DateTimeOffset.UtcNow + TimeSpan.FromMinutes(3);
        while (DateTimeOffset.UtcNow < deadline)
        {
            await Task.Delay(TimeSpan.FromSeconds(3), cancellationToken);
            if ((await runner.RunAsync(docker, ["info"], cancellationToken: cancellationToken)).Succeeded)
            {
                return;
            }
        }

        throw new LauncherException("Docker Desktop が 3 分たっても動き出しません");
    }
}

/// <summary>The MySQL root password of the installed game_master.</summary>
/// <remarks>
/// MySQL sets it when it creates the database volume, so it's kept as long as the volume:
/// uninstalling game_master removes it only together with the database.
/// </remarks>
public static class GameMasterSecret
{
    public static string PathIn(AppPaths paths) => Path.Combine(paths.DataDirectory, "secrets", "mysql-root-password");

    public static string? Load(AppPaths paths)
    {
        var path = PathIn(paths);
        return File.Exists(path) ? File.ReadAllText(path).Trim() : null;
    }

    public static string LoadOrCreate(AppPaths paths)
    {
        if (Load(paths) is { } existing)
        {
            return existing;
        }

        var path = PathIn(paths);
        System.IO.Directory.CreateDirectory(System.IO.Path.GetDirectoryName(path)!);
        var password = Convert.ToHexString(RandomNumberGenerator.GetBytes(24));
        using (var file = File.Create(path))
        {
            if (!OperatingSystem.IsWindows())
            {
                File.SetUnixFileMode(file.SafeFileHandle, UnixFileMode.UserRead | UnixFileMode.UserWrite);
            }

            file.Write(System.Text.Encoding.UTF8.GetBytes(password));
        }

        return password;
    }

    public static void Delete(AppPaths paths) => File.Delete(PathIn(paths));
}
