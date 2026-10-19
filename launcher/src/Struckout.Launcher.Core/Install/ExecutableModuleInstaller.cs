using Struckout.Launcher.Core.Modules;
using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.Core.Install;

/// <summary>ball_tracker and touchpanel: a single binary in the release.</summary>
/// <param name="binaryName">The name cargo gives the binary.</param>
internal sealed class ExecutableModuleInstaller(
    ModuleId id, string binaryName, HostPlatform platform, AppPaths paths, HttpDownloader downloader) : IModuleInstaller
{
    public async Task<ModuleInstallation> InstallAsync(InstallContext context, ModuleInstallation? previous)
    {
        var asset = ReleaseAssets.Pick(context.Release, platform, id.Slug());
        var directory = paths.ModuleDirectory(id.Slug());
        var executable = Path.Combine(directory, platform.ExecutableName(binaryName));
        context.Log.Report($"{id.Slug()}: {asset.Name} をダウンロードします");
        await downloader.DownloadAsync(asset.DownloadUrl, executable, context.Download, context.CancellationToken);
        if (!OperatingSystem.IsWindows())
        {
            File.SetUnixFileMode(
                executable,
                UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute
                | UnixFileMode.GroupRead | UnixFileMode.GroupExecute
                | UnixFileMode.OtherRead | UnixFileMode.OtherExecute);
        }

        return new ExecutableInstallation
        {
            ReleaseTag = context.Release.Tag,
            ReleasePublishedAt = context.Release.PublishedAt,
            Directory = directory,
            Executable = executable,
        };
    }

    public Task UninstallAsync(
        ModuleInstallation installation,
        IReadOnlyDictionary<PrerequisiteId, string> tools,
        UninstallOptions options,
        IProgress<string> log,
        CancellationToken cancellationToken)
    {
        log.Report($"{id.Slug()}: {installation.Directory} を消します");
        if (Directory.Exists(installation.Directory))
        {
            Directory.Delete(installation.Directory, recursive: true);
        }

        return Task.CompletedTask;
    }
}
