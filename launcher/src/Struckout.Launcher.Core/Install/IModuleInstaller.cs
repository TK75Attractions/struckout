using Struckout.Launcher.Core.GitHub;
using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.Core.Install;

/// <param name="Tools">The executables of the prerequisites of the module.</param>
internal sealed record InstallContext(
    Release Release,
    IReadOnlyDictionary<PrerequisiteId, string> Tools,
    IProgress<string> Log,
    IProgress<DownloadProgress>? Download,
    CancellationToken CancellationToken);

public sealed record UninstallOptions
{
    /// <summary>Also delete game_master's database (the rankings).</summary>
    public bool DeleteDatabase { get; init; }
}

/// <summary>Puts one module on this PC and takes it off again.</summary>
internal interface IModuleInstaller
{
    /// <param name="previous">What the last install of the module left, if any.</param>
    /// <exception cref="LauncherException">The install failed.</exception>
    Task<ModuleInstallation> InstallAsync(InstallContext context, ModuleInstallation? previous);

    /// <param name="tools">The prerequisites that are still on this PC.</param>
    /// <exception cref="LauncherException">The uninstall failed.</exception>
    Task UninstallAsync(
        ModuleInstallation installation,
        IReadOnlyDictionary<PrerequisiteId, string> tools,
        UninstallOptions options,
        IProgress<string> log,
        CancellationToken cancellationToken);
}

internal static class ReleaseAssets
{
    /// <summary>The asset <c>&lt;slug&gt;-&lt;suffix&gt;&lt;extension&gt;</c> for the best suffix this host runs.</summary>
    /// <exception cref="LauncherException">The release has none for this host.</exception>
    public static ReleaseAsset Pick(Release release, HostPlatform platform, string slug, string extension = "")
    {
        foreach (var suffix in platform.ReleaseAssetSuffixes)
        {
            if (release.FindAsset($"{slug}-{suffix}{extension}") is { } asset)
            {
                return asset;
            }
        }

        throw new LauncherException(
            $"リリース {release.Tag} に、この PC ({platform.Os}, {platform.Architecture}) で動く {slug} がありません");
    }

    /// <summary>
    /// Downloads <paramref name="asset"/> and checks that it has the hash GitHub reported, so that a broken or
    /// tampered file is never installed.
    /// </summary>
    /// <remarks>
    /// A <paramref name="destination"/> that already has the hash is kept instead. The install may fail after the
    /// download, as camera.apk does when a phone is not authorized, and the retry then needn't download the same
    /// file again. A file of another release has another hash, so it is replaced.
    /// </remarks>
    /// <param name="slug">The module, to prefix the messages with.</param>
    /// <exception cref="LauncherException">GitHub reports no hash, or the download failed or doesn't match.</exception>
    public static async Task DownloadAsync(
        HttpDownloader downloader, string slug, ReleaseAsset asset, string destination, InstallContext context)
    {
        var digest = asset.Digest ?? throw new LauncherException(
            $"リリース {context.Release.Tag} の {asset.Name} にハッシュが無いので、壊れていないか確かめられません");
        if (File.Exists(destination) && await Sha256Digest.OfFileAsync(destination, context.CancellationToken) == digest)
        {
            context.Log.Report($"{slug}: ダウンロード済みの {asset.Name} を使います");
            // The progress bar shows the total only when every download has reported its size.
            var size = new FileInfo(destination).Length;
            context.Download?.Report(new DownloadProgress(size, size));
            return;
        }

        context.Log.Report($"{slug}: {asset.Name} をダウンロードします");
        await downloader.DownloadAsync(
            asset.DownloadUrl, destination, digest, context.Download, context.Log, context.CancellationToken);
    }
}
