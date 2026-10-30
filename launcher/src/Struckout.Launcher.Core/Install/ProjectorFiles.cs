using System.IO.Compression;
using System.Text.Json;
using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.Core.Install;

/// <summary>Validates and replaces the whole Unity player so a failed update keeps the previous files.</summary>
internal static class ProjectorFiles
{
    internal const string ManifestName = ".struckout-manifest.json";

    internal static IEnumerable<string> RequiredFiles(OperatingSystemKind os)
    {
        yield return ProjectorInstaller.ExecutableInArchive(os);
        var root = os switch
        {
            OperatingSystemKind.Windows => "StandaloneWindows64",
            OperatingSystemKind.Linux => "StandaloneLinux64",
            OperatingSystemKind.MacOS => "StandaloneOSX/StandaloneOSX.app/Contents",
            _ => throw new ArgumentOutOfRangeException(nameof(os)),
        };
        yield return root + (os switch
        {
            OperatingSystemKind.Windows => "/UnityPlayer.dll",
            OperatingSystemKind.Linux => "/UnityPlayer.so",
            _ => "/Frameworks/UnityPlayer.dylib",
        });
        var data = root + (os switch
        {
            OperatingSystemKind.Windows => "/StandaloneWindows64_Data",
            OperatingSystemKind.Linux => "/StandaloneLinux64_Data",
            _ => "/Resources/Data",
        });
        yield return data + "/globalgamemanagers";
        // ProjectSettings.asset sets scriptingBackend only for Android; Standalone uses the Mono default.
        // release.yaml's unity-build does not override it; ReleaseBuild.BuildWindowsArm64 also selects Mono2x.
        // Requiring the assembly also verifies its containing Data and Managed directories.
        yield return data + "/Managed/Assembly-CSharp.dll";
    }

    internal static async Task ExtractAsync(string archive, string extracting, OperatingSystemKind os, CancellationToken cancellationToken)
    {
        if (Directory.Exists(extracting))
        {
            Directory.Delete(extracting, recursive: true);
        }

        try
        {
            // .NET restores Unix permissions from the zip, including the player's executable bit.
            await ZipFile.ExtractToDirectoryAsync(archive, extracting, cancellationToken);
            using var zip = ZipFile.OpenRead(archive);
            ValidateArchive(zip, extracting);
            foreach (var file in RequiredFiles(os))
            {
                RequireFile(extracting, file);
            }

            // Keep the inventory with the player, rather than extending the shared ExecutableInstallation:
            // directory renames and crash recovery then restore the matching manifest as well as the files.
            var files = Directory.EnumerateFiles(extracting, "*", SearchOption.AllDirectories)
                .ToDictionary(file => Path.GetRelativePath(extracting, file).Replace('\\', '/'), file => new FileInfo(file).Length);
            if (files.ContainsKey(ManifestName))
            {
                throw new LauncherException($"projector の配布物に予約済みの {ManifestName} が含まれています");
            }

            await File.WriteAllTextAsync(Path.Combine(extracting, ManifestName), JsonSerializer.Serialize(files), cancellationToken);
        }
        catch (InvalidDataException e)
        {
            throw new LauncherException($"projector の zip が壊れています: {e.Message}", e);
        }
    }

    internal static void ValidateArchive(ZipArchive zip, string directory)
    {
        foreach (var entry in zip.Entries)
        {
            var path = ResolvePath(directory, entry.FullName);
            if (entry.FullName.EndsWith('/'))
            {
                if (!Directory.Exists(path))
                {
                    throw Missing(entry.FullName);
                }
            }
            else
            {
                RequireFile(directory, entry.FullName, entry.Length);
            }
        }
    }

    internal static void ValidateForLaunch(string directory, OperatingSystemKind os, IProgress<string> log)
    {
        try
        {
            Recover(directory);
            var manifest = Path.Combine(directory, ManifestName);
            if (!File.Exists(manifest))
            {
                foreach (var file in RequiredFiles(os))
                {
                    RequireFile(directory, file);
                }

                log.Report("projector: ファイルの一覧が未記録のため必須ファイルのみ検査しました。次の更新でファイルの一覧を記録します");
                return;
            }

            var files = JsonSerializer.Deserialize<Dictionary<string, long>>(File.ReadAllText(manifest));
            if (files is null || files.Count == 0 || files.Values.Any(size => size < 0))
            {
                throw new JsonException("Invalid file inventory");
            }

            foreach (var (file, size) in files)
            {
                RequireFile(directory, file, size);
            }

            foreach (var file in RequiredFiles(os))
            {
                RequireFile(directory, file);
            }
        }
        catch (Exception e) when (e is JsonException or IOException or UnauthorizedAccessException)
        {
            throw new LauncherException($"projector のファイルを検査・復旧できません。インストール画面で projector を入れ直してください: {e.Message}", e);
        }
    }

    internal static void Recover(string directory)
    {
        if (!Directory.Exists(directory) && Directory.Exists(directory + ".old"))
        {
            Directory.Move(directory + ".old", directory);
        }
    }

    /// <summary>The move delegate lets tests reproduce a failed second rename without OS-specific locks.</summary>
    internal static void Replace(string directory, Action<string, string>? move = null)
    {
        move ??= Directory.Move;
        Recover(directory);
        var old = directory + ".old";
        if (Directory.Exists(old))
        {
            Directory.Delete(old, recursive: true);
        }

        var hadPrevious = Directory.Exists(directory);
        if (hadPrevious)
        {
            move(directory, old);
        }

        try
        {
            move(directory + ".new", directory);
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            if (hadPrevious)
            {
                try
                {
                    move(old, directory);
                }
                catch (Exception recovery) when (recovery is IOException or UnauthorizedAccessException)
                {
                    throw new LauncherException($"projector の更新と復旧に失敗しました。元のファイルは {old} に残っています: {recovery.Message}",
                        new AggregateException(e, recovery));
                }
            }

            throw new LauncherException($"projector を入れ替えられません。元のインストールを保持しました: {e.Message}", e);
        }
    }

    private static string ResolvePath(string directory, string relative)
    {
        var root = Path.GetFullPath(directory) + Path.DirectorySeparatorChar;
        var path = Path.GetFullPath(Path.Combine(directory, relative.Replace('\\', '/')));
        if (!path.StartsWith(root, OperatingSystem.IsWindows() ? StringComparison.OrdinalIgnoreCase : StringComparison.Ordinal))
        {
            throw new LauncherException($"projector のファイルパス {relative} が不正です。インストール画面で projector を入れ直してください");
        }

        return path;
    }

    private static void RequireFile(string directory, string relative, long? size = null)
    {
        var file = new FileInfo(ResolvePath(directory, relative));
        if (!file.Exists)
        {
            throw Missing(relative);
        }

        if (size is not null && file.Length != size)
        {
            throw new LauncherException($"projector の {relative} のサイズが一致しません。インストール画面で projector を入れ直してください");
        }
    }

    private static LauncherException Missing(string relative) =>
        new($"projector の {relative} がありません。インストール画面で projector を入れ直してください");
}
