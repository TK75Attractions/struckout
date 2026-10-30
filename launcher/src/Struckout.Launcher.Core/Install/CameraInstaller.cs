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
        var existing = new SortedSet<string>((previous as ApkInstallation)?.ExistingDevices ?? [], StringComparer.Ordinal);
        foreach (var serial in connected)
        {
            if (cameras.Cameras.All(c => c.Serial != serial))
            {
                context.Log.Report($"{slug}: {serial} は {paths.CamerasFile} に無いので入れません");
                continue;
            }

            context.Log.Report($"{slug}: {serial} に入れます");
            if (await InstallOnDeviceAsync(adb, serial, apk, cameras.Package, context.Log, context.CancellationToken))
            {
                existing.Add(serial);
            }
            else
            {
                existing.Remove(serial);
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
            ExistingDevices = existing.ToList(),
        };
    }

    /// <returns>Whether a differently signed existing app was retained to preserve its calibration.</returns>
    internal static async Task<bool> InstallOnDeviceAsync(
        Adb adb, string serial, string apk, string package, IProgress<string> log, CancellationToken cancellationToken)
    {
        var result = await adb.RunAsync(serial, ["install", "-r", "-g", apk], cancellationToken);
        if (result.Output.Contains("INSTALL_FAILED_UPDATE_INCOMPATIBLE", StringComparison.Ordinal))
        {
            var path = await adb.RunAsync(serial, ["shell", "pm", "path", package], cancellationToken);
            if (path.Succeeded && path.Output.Split('\n').Any(line => line.Trim().StartsWith("package:", StringComparison.Ordinal)))
            {
                log.Report($"camera: {serial}: 署名が違う既存のアプリを使います。リリースの版には更新されません");
                return true;
            }
        }

        if (!result.Succeeded || !result.Output.Contains("Success", StringComparison.Ordinal))
        {
            throw new LauncherException($"{serial} にアプリを入れられません:\n{result.Output.Trim()}");
        }

        return false;
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

                // The launcher didn't install it, and removing it would lose the calibration in the app.
                if (apk.ExistingDevices.Contains(serial))
                {
                    log.Report($"{slug}: {serial} のアプリはランチャーが入れたものではないので残します");
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
