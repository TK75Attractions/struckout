using Struckout.Launcher.Core.Modules;
using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.Core.Install;

/// <summary>The camera app: <c>camera.apk</c> of the release, installed on the phones in <c>cameras.json</c>.</summary>
internal sealed class CameraInstaller(AppPaths paths, HttpDownloader downloader, ICommandRunner runner) : IModuleInstaller
{
    /// <summary>The asset the "camera-build" job of <c>.github/workflows/release.yaml</c> uploads.</summary>
    internal const string AssetName = "camera.apk";

    public async Task<ModuleInstallation> InstallAsync(InstallContext context, ModuleInstallation? previous)
    {
        var slug = ModuleId.Camera.Slug();
        var asset = context.Release.FindAsset(AssetName)
            ?? throw new LauncherException($"リリース {context.Release.Tag} に {AssetName} がありません");
        var directory = paths.ModuleDirectory(slug);
        var apk = Path.Combine(directory, AssetName);
        context.Log.Report($"{slug}: {AssetName} をダウンロードします");
        await ReleaseAssets.DownloadAsync(downloader, asset, apk, context);

        var cameras = CamerasFile.Load(paths.CamerasFile);
        var adb = new Adb(context.Tools[PrerequisiteId.Adb], runner);
        var connected = await adb.DevicesAsync(context.CancellationToken);
        var installed = new SortedSet<string>((previous as ApkInstallation)?.Devices ?? [], StringComparer.Ordinal);
        foreach (var serial in connected)
        {
            if (cameras.Cameras.All(c => c.Serial != serial))
            {
                context.Log.Report($"{slug}: {serial} は {paths.CamerasFile} に無いので入れません");
                continue;
            }

            context.Log.Report($"{slug}: {serial} に入れます");
            var result = await adb.RunAsync(serial, ["install", "-r", apk], context.CancellationToken);
            if (result.Output.Contains("INSTALL_FAILED_UPDATE_INCOMPATIBLE", StringComparison.Ordinal))
            {
                throw new LauncherException(
                    $"{serial} には、署名の違うカメラアプリ (Android Studio から入れたものなど) が入っています。"
                    + $"端末のアプリを消してからやり直してください (adb -s {serial} uninstall {cameras.Package})");
            }

            if (!result.Succeeded || !result.Output.Contains("Success", StringComparison.Ordinal))
            {
                throw new LauncherException($"{serial} にアプリを入れられません:\n{result.Output.Trim()}");
            }

            installed.Add(serial);
        }

        if (installed.Count == 0)
        {
            context.Log.Report(
                $"{slug}: cameras.json のスマホがつながっていないので、アプリは端末に入れていません。"
                + "つないでから、もう一度インストールしてください");
        }

        return new ApkInstallation
        {
            ReleaseTag = context.Release.Tag,
            ReleasePublishedAt = context.Release.PublishedAt,
            Directory = directory,
            Package = cameras.Package,
            Devices = installed.ToList(),
        };
    }

    public async Task UninstallAsync(
        ModuleInstallation installation,
        IReadOnlyDictionary<PrerequisiteId, string> tools,
        UninstallOptions options,
        IProgress<string> log,
        CancellationToken cancellationToken)
    {
        var slug = ModuleId.Camera.Slug();
        var apk = (ApkInstallation)installation;
        if (tools.TryGetValue(PrerequisiteId.Adb, out var executable))
        {
            var adb = new Adb(executable, runner);
            var connected = await adb.DevicesAsync(cancellationToken);
            foreach (var serial in apk.Devices)
            {
                if (!connected.Contains(serial))
                {
                    log.Report($"{slug}: {serial} がつながっていないので、端末のアプリは残ります");
                    continue;
                }

                log.Report($"{slug}: {serial} からアプリを消します");
                await adb.RunAsync(serial, ["uninstall", apk.Package], cancellationToken);
            }
        }
        else
        {
            log.Report($"{slug}: adb が無いので、端末のアプリは残ります");
        }

        if (Directory.Exists(installation.Directory))
        {
            Directory.Delete(installation.Directory, recursive: true);
        }
    }
}
