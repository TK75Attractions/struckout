using System.IO.Compression;
using Struckout.Launcher.Core.Modules;
using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.Core.Install;

/// <summary>Finds the prerequisites of the modules, and installs the missing ones.</summary>
public sealed class PrerequisiteInstaller(
    HostPlatform platform, AppPaths paths, ToolLocator locator, ICommandRunner runner, HttpDownloader downloader)
{
    public static IReadOnlyList<PrerequisiteId> RequiredBy(ModuleId id) => id switch
    {
        ModuleId.BallTracker => [PrerequisiteId.Ffmpeg],
        ModuleId.Camera => [PrerequisiteId.Adb],
        ModuleId.GameMaster => [PrerequisiteId.Docker],
        ModuleId.Projector or ModuleId.Touchpanel => [],
        _ => throw new ArgumentOutOfRangeException(nameof(id), id, null),
    };

    public static string DisplayName(PrerequisiteId id) => id switch
    {
        PrerequisiteId.Docker => "Docker",
        PrerequisiteId.Adb => "Android Platform Tools (adb)",
        PrerequisiteId.Ffmpeg => "FFmpeg",
        _ => throw new ArgumentOutOfRangeException(nameof(id), id, null),
    };

    /// <summary>How we install <paramref name="id"/> on <paramref name="platform"/>.</summary>
    internal static InstallMethod MethodFor(PrerequisiteId id, HostPlatform platform) => (id, platform.Os) switch
    {
        (PrerequisiteId.Docker, OperatingSystemKind.Windows) => new PackageInstall(PackageManager.Winget, "Docker.DockerDesktop"),
        (PrerequisiteId.Docker, OperatingSystemKind.MacOS) => new PackageInstall(PackageManager.HomebrewCask, "docker-desktop"),
        (PrerequisiteId.Docker, OperatingSystemKind.Linux) => new ManualInstall(
            "Docker Engine を https://docs.docker.com/engine/install/ の手順で入れ、"
            + "sudo なしで docker を使えるようにしてください (docker グループに入る)"),
        (PrerequisiteId.Ffmpeg, OperatingSystemKind.Windows) => new PackageInstall(PackageManager.Winget, "Gyan.FFmpeg"),
        (PrerequisiteId.Ffmpeg, OperatingSystemKind.MacOS) => new PackageInstall(PackageManager.Homebrew, "ffmpeg"),
        (PrerequisiteId.Ffmpeg, OperatingSystemKind.Linux) => new ManualInstall(
            "ディストリビューションのパッケージで ffmpeg を入れてください (例: sudo apt install ffmpeg)"),
        // https://developer.android.com/tools/releases/platform-tools
        (PrerequisiteId.Adb, var os) => new PlatformToolsDownload(new Uri(
            "https://dl.google.com/android/repository/platform-tools-latest-" + os switch
            {
                OperatingSystemKind.Windows => "windows",
                OperatingSystemKind.MacOS => "darwin",
                _ => "linux",
            } + ".zip")),
        _ => throw new ArgumentOutOfRangeException(nameof(id), id, null),
    };

    /// <summary>
    /// Makes sure <paramref name="id"/> is on this PC and records how it got here in <paramref name="state"/>.
    /// </summary>
    /// <returns>The executable.</returns>
    /// <exception cref="LauncherException">It's missing and can't be installed.</exception>
    public async Task<string> EnsureAsync(
        PrerequisiteId id, InstallState state, IProgress<string> log, CancellationToken cancellationToken)
    {
        var name = DisplayName(id);
        if (locator.Find(id) is { } found)
        {
            // Keep the record of a tool we installed, even if it's now also on PATH.
            if (!state.Prerequisites.ContainsKey(id))
            {
                state.Prerequisites[id] = new FoundPrerequisite(found);
                log.Report($"{name}: 入っているものを使います ({found})");
            }

            return found;
        }

        state.Prerequisites[id] = await InstallAsync(id, log, cancellationToken);
        return locator.Find(id) ?? throw new LauncherException(
            $"{name} を入れましたが見つかりません。PC を再起動してからやり直してください");
    }

    /// <summary>Removes <paramref name="id"/> if we installed it.</summary>
    /// <exception cref="LauncherException">The package manager failed.</exception>
    public async Task RemoveAsync(
        PrerequisiteId id, PrerequisiteOrigin origin, IProgress<string> log, CancellationToken cancellationToken)
    {
        var name = DisplayName(id);
        switch (origin)
        {
            case FoundPrerequisite:
                log.Report($"{name}: もとから入っていたので残します");
                break;
            case PackageManagerPrerequisite(var manager, var package):
                log.Report($"{name}: アンインストールします");
                await RunPackageManagerAsync(manager, package, uninstall: true, cancellationToken);
                break;
            case DownloadedPrerequisite(var directory):
                log.Report($"{name}: {directory} を消します");
                if (Directory.Exists(directory))
                {
                    Directory.Delete(directory, recursive: true);
                }

                break;
            default:
                throw new ArgumentOutOfRangeException(nameof(origin), origin, null);
        }
    }

    private async Task<PrerequisiteOrigin> InstallAsync(
        PrerequisiteId id, IProgress<string> log, CancellationToken cancellationToken)
    {
        var name = DisplayName(id);
        switch (MethodFor(id, platform))
        {
            case PackageInstall(var manager, var package):
                log.Report($"{name}: {manager} で {package} を入れます (数分かかることがあります)");
                await RunPackageManagerAsync(manager, package, uninstall: false, cancellationToken);
                return new PackageManagerPrerequisite(manager, package);
            case PlatformToolsDownload(var url):
                log.Report($"{name}: {url} からダウンロードします");
                var archive = Path.Combine(paths.ToolsDirectory, "platform-tools.zip");
                // Google publishes no hash for the "latest" archive, so this relies on HTTPS alone.
                await downloader.DownloadAsync(url, archive, expected: null, progress: null, cancellationToken);
                var directory = locator.DownloadedPlatformTools;
                if (Directory.Exists(directory))
                {
                    Directory.Delete(directory, recursive: true);
                }

                try
                {
                    // The archive has a top-level platform-tools/ directory. .NET restores the Unix permissions.
                    await ZipFile.ExtractToDirectoryAsync(archive, paths.ToolsDirectory, cancellationToken);
                }
                catch (InvalidDataException e)
                {
                    throw new LauncherException($"{url} が壊れています: {e.Message}", e);
                }

                File.Delete(archive);
                return new DownloadedPrerequisite(directory);
            case ManualInstall(var instruction):
                throw new LauncherException($"{name} が見つかりません。{instruction}");
            default:
                throw new InvalidOperationException($"unknown install method for {id}");
        }
    }

    private async Task RunPackageManagerAsync(
        PackageManager manager, string package, bool uninstall, CancellationToken cancellationToken)
    {
        var (executable, arguments) = manager switch
        {
            PackageManager.Winget => (
                FindWinget(),
                uninstall
                    ? new[] { "uninstall", "--exact", "--id", package, "--silent", "--disable-interactivity" }
                    : ["install", "--exact", "--id", package, "--silent", "--disable-interactivity",
                        "--accept-package-agreements", "--accept-source-agreements"]),
            PackageManager.Homebrew => (FindBrew(), [uninstall ? "uninstall" : "install", package]),
            PackageManager.HomebrewCask => (FindBrew(), [uninstall ? "uninstall" : "install", "--cask", package]),
            _ => throw new ArgumentOutOfRangeException(nameof(manager), manager, null),
        };
        var result = await runner.RunAsync(executable, arguments, cancellationToken: cancellationToken);
        if (!result.Succeeded)
        {
            throw new LauncherException(
                $"{executable} {string.Join(' ', arguments)} が失敗しました (終了コード {result.ExitCode}):\n"
                + result.Output.Trim());
        }
    }

    private static string FindWinget() =>
        ExecutableSearch.OnPath("winget.exe")
        ?? throw new LauncherException("winget が見つかりません。Microsoft Store で「アプリ インストーラー」を入れてください");

    private static string FindBrew() =>
        ExecutableSearch.OnPath("brew")
        ?? ExecutableSearch.FirstExisting(["/opt/homebrew/bin/brew", "/usr/local/bin/brew"])
        ?? throw new LauncherException("Homebrew が見つかりません。https://brew.sh の手順で入れてください");
}

internal abstract record InstallMethod;

internal sealed record PackageInstall(PackageManager Manager, string PackageId) : InstallMethod;

internal sealed record PlatformToolsDownload(Uri Url) : InstallMethod;

/// <summary>We can't install it without root, so the user does it by <paramref name="Instruction"/>.</summary>
internal sealed record ManualInstall(string Instruction) : InstallMethod;
