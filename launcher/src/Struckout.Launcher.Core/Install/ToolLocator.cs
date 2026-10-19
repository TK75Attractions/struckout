using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.Core.Install;

/// <summary>Finds the executables of the prerequisites, including where installers put them off <c>PATH</c>.</summary>
/// <remarks>
/// winget and Homebrew update <c>PATH</c> only for new shells, so right after installing them the launcher
/// has to look in their install directories.
/// </remarks>
public sealed class ToolLocator(HostPlatform platform, AppPaths paths)
{
    public string? Find(PrerequisiteId id) => id switch
    {
        PrerequisiteId.Docker => FindDocker(),
        PrerequisiteId.Adb => FindAdb(),
        PrerequisiteId.Ffmpeg => FindFfmpeg(),
        _ => throw new ArgumentOutOfRangeException(nameof(id), id, null),
    };

    /// <summary>Where we extract Android Platform Tools when adb isn't installed.</summary>
    public string DownloadedPlatformTools => Path.Combine(paths.ToolsDirectory, "platform-tools");

    private string? FindDocker()
    {
        var name = platform.ExecutableName("docker");
        return ExecutableSearch.OnPath(name) ?? ExecutableSearch.FirstExisting(platform.Os switch
        {
            OperatingSystemKind.Windows =>
            [
                Path.Combine(
                    Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles),
                    "Docker", "Docker", "resources", "bin", name),
            ],
            OperatingSystemKind.MacOS =>
            [
                "/usr/local/bin/docker",
                "/opt/homebrew/bin/docker",
                "/Applications/Docker.app/Contents/Resources/bin/docker",
            ],
            _ => [],
        });
    }

    private string? FindAdb()
    {
        var name = platform.ExecutableName("adb");
        var home = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
        var defaultSdk = platform.Os switch
        {
            OperatingSystemKind.Windows => Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Android", "Sdk"),
            OperatingSystemKind.MacOS => Path.Combine(home, "Library", "Android", "sdk"),
            _ => Path.Combine(home, "Android", "Sdk"),
        };
        var sdks = new[]
        {
            Environment.GetEnvironmentVariable("ANDROID_HOME"),
            Environment.GetEnvironmentVariable("ANDROID_SDK_ROOT"),
            defaultSdk,
        };
        return ExecutableSearch.OnPath(name)
            ?? ExecutableSearch.FirstExisting(sdks
                .Where(sdk => !string.IsNullOrEmpty(sdk))
                .Select(sdk => Path.Combine(sdk!, "platform-tools", name))
                .Append(Path.Combine(DownloadedPlatformTools, name)));
    }

    private string? FindFfmpeg()
    {
        var name = platform.ExecutableName("ffmpeg");
        var found = ExecutableSearch.OnPath(name);
        if (found is not null)
        {
            return found;
        }

        switch (platform.Os)
        {
            case OperatingSystemKind.Windows:
                var winget = Path.Combine(
                    Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Microsoft", "WinGet");
                var link = Path.Combine(winget, "Links", name);
                if (File.Exists(link))
                {
                    return link;
                }

                var packages = Path.Combine(winget, "Packages");
                if (!Directory.Exists(packages))
                {
                    return null;
                }

                return Directory.EnumerateDirectories(packages, "Gyan.FFmpeg_*")
                    .SelectMany(package => Directory.EnumerateFiles(package, name, SearchOption.AllDirectories))
                    .Order(StringComparer.Ordinal)
                    .LastOrDefault();
            case OperatingSystemKind.MacOS:
                return ExecutableSearch.FirstExisting(["/opt/homebrew/bin/ffmpeg", "/usr/local/bin/ffmpeg"]);
            default:
                return null;
        }
    }
}
