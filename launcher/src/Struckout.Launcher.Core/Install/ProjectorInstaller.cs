using Struckout.Launcher.Core.Modules;
using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.Core.Install;

/// <summary>projector: a zip of the Unity player.</summary>
internal sealed class ProjectorInstaller(HostPlatform platform, AppPaths paths, HttpDownloader downloader) : IModuleInstaller
{
    /// <summary>The player inside the zip.</summary>
    /// <remarks>
    /// The top directory is the build target that the "unity-build" job of
    /// <c>.github/workflows/release.yaml</c> passes to game-ci/unity-builder.
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
        var directory = paths.ModuleDirectory(slug);
        ProjectorFiles.Recover(directory);
        var asset = ReleaseAssets.Pick(context.Release, platform, slug, ".zip");
        var archive = directory + ".zip";
        var extracting = directory + ".new";
        await ReleaseAssets.DownloadAsync(downloader, slug, asset, archive, context);

        context.Log.Report($"{slug}: 展開します");
        await ProjectorFiles.ExtractAsync(archive, extracting, platform.Os, context.CancellationToken);
        File.Delete(archive);
        context.CancellationToken.ThrowIfCancellationRequested();
        ProjectorFiles.Replace(directory);
        try
        {
            if (Directory.Exists(directory + ".old"))
            {
                Directory.Delete(directory + ".old", recursive: true);
            }
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            // The replacement succeeded. Record it even if removing the backup must wait until the next update.
            context.Log.Report($"projector: 更新は完了しましたが、{directory}.old を削除できません: {e.Message}");
        }

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
        // Also remove staging and backup files so a later install cannot recover an uninstalled player.
        foreach (var directory in new[] { installation.Directory, installation.Directory + ".new", installation.Directory + ".old" })
        {
            if (Directory.Exists(directory))
            {
                Directory.Delete(directory, recursive: true);
            }
        }

        File.Delete(installation.Directory + ".zip");
        // An unfinished download is kept to be continued by the next install, which won't come.
        HttpDownloader.DeletePartials(installation.Directory + ".zip");
        return Task.CompletedTask;
    }
}
