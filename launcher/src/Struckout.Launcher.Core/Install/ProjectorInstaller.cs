using System.IO.Compression;
using Struckout.Launcher.Core.Modules;
using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.Core.Install;

/// <summary>projector: a zip of the Unity player.</summary>
internal sealed class ProjectorInstaller(HostPlatform platform, AppPaths paths, HttpDownloader downloader) : IModuleInstaller
{
    /// <summary>The player inside the zip.</summary>
    /// <remarks>
    /// The top directory is the build target that the "unity-build" job of
    /// <c>.github/workflows/nightly_release.yaml</c> passes to game-ci/unity-builder.
    /// </remarks>
    internal static string ExecutableInArchive(OperatingSystemKind os) => os switch
    {
        OperatingSystemKind.Windows => Path.Combine("StandaloneWindows64", "StandaloneWindows64.exe"),
        OperatingSystemKind.MacOS => Path.Combine(
            "StandaloneOSX", "StandaloneOSX.app", "Contents", "MacOS", "StruckOut"),
        OperatingSystemKind.Linux => Path.Combine("StandaloneLinux64", "StandaloneLinux64.x86_64"),
        _ => throw new ArgumentOutOfRangeException(nameof(os), os, null),
    };

    public async Task<ModuleInstallation> InstallAsync(InstallContext context, ModuleInstallation? previous)
    {
        var slug = ModuleId.Projector.Slug();
        var asset = ReleaseAssets.Pick(context.Release, platform, slug, ".zip");
        var directory = paths.ModuleDirectory(slug);
        var archive = directory + ".zip";
        var extracting = directory + ".new";
        context.Log.Report($"{slug}: {asset.Name} をダウンロードします");
        await downloader.DownloadAsync(asset.DownloadUrl, archive, context.Download, context.CancellationToken);

        context.Log.Report($"{slug}: 展開します");
        if (Directory.Exists(extracting))
        {
            Directory.Delete(extracting, recursive: true);
        }

        try
        {
            // .NET restores the Unix permissions recorded in the zip, so the player stays executable.
            await ZipFile.ExtractToDirectoryAsync(archive, extracting, context.CancellationToken);
        }
        catch (InvalidDataException e)
        {
            throw new LauncherException($"{asset.Name} が壊れています: {e.Message}", e);
        }

        File.Delete(archive);
        if (!File.Exists(Path.Combine(extracting, ExecutableInArchive(platform.Os))))
        {
            throw new LauncherException(
                $"{asset.Name} に {ExecutableInArchive(platform.Os)} がありません (配布物の形が変わった?)");
        }

        if (Directory.Exists(directory))
        {
            Directory.Delete(directory, recursive: true);
        }

        Directory.Move(extracting, directory);
        return new ExecutableInstallation
        {
            ReleaseTag = context.Release.Tag,
            ReleasePublishedAt = context.Release.PublishedAt,
            Directory = directory,
            Executable = Path.Combine(directory, ExecutableInArchive(platform.Os)),
        };
    }

    public Task UninstallAsync(
        ModuleInstallation installation,
        IReadOnlyDictionary<PrerequisiteId, string> tools,
        UninstallOptions options,
        IProgress<string> log,
        CancellationToken cancellationToken)
    {
        log.Report($"projector: {installation.Directory} を消します");
        if (Directory.Exists(installation.Directory))
        {
            Directory.Delete(installation.Directory, recursive: true);
        }

        return Task.CompletedTask;
    }
}
