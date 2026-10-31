using System.Net;
using System.Security.Cryptography;
using System.Text;
using Struckout.Launcher.Core.GitHub;
using Struckout.Launcher.Core.Install;
using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.Core.Tests;

public sealed class CameraInstallerTests : IDisposable
{
    private const string Serial = "166810122536";

    private readonly string directory = Path.Combine(Path.GetTempPath(), $"camera-install-{Guid.NewGuid()}");
    private readonly AppPaths paths;
    private readonly Apks server = new();
    private readonly Phone phone = new();
    private readonly Messages log = new();
    private readonly CameraInstaller installer;

    public CameraInstallerTests()
    {
        paths = new AppPaths(Path.Combine(directory, "config"), Path.Combine(directory, "data"));
        Directory.CreateDirectory(paths.ConfigDirectory);
        File.WriteAllText(paths.CamerasFile, $$"""
            { "package": "com.taichi765.struckoutCameraApp.highspeed", "cameras": [
              { "serial": "{{Serial}}", "name": "left", "output": "network", "focal_ratio": 0.97, "pose": [] }
            ] }
            """);
        installer = new CameraInstaller(paths, new HttpDownloader(new HttpClient(server)), phone);
    }

    public void Dispose()
    {
        if (Directory.Exists(directory))
        {
            Directory.Delete(directory, recursive: true);
        }
    }

    [Fact]
    public async Task ReusesTheApkWhenTheInstallIsRetried()
    {
        phone.Authorized = false;
        await installer.InstallAsync(Context(Release("2026-10-31.0", "first")), null);
        phone.Authorized = true;
        phone.InstallOutput = "Failure [INSTALL_FAILED_INSUFFICIENT_STORAGE]";
        await Assert.ThrowsAsync<LauncherException>(() =>
            installer.InstallAsync(Context(Release("2026-10-31.0", "first")), null));
        phone.InstallOutput = "Success";

        var installation = await installer.InstallAsync(Context(Release("2026-10-31.0", "first")), null);

        Assert.Equal([Serial], Assert.IsType<ApkInstallation>(installation).Devices);
        Assert.Equal(1, server.Requests);
        Assert.Equal(2, log.Values.Count(m => m == "camera: ダウンロード済みの camera.apk を使います"));
    }

    [Fact]
    public async Task DownloadsTheApkOfAnotherRelease()
    {
        await installer.InstallAsync(Context(Release("2026-10-31.0", "first")), null);

        await installer.InstallAsync(Context(Release("2026-11-01.0", "second")), null);

        Assert.Equal(2, server.Requests);
        Assert.Equal("second", File.ReadAllText(Path.Combine(paths.ModuleDirectory("camera"), "camera.apk")));
    }

    [Fact]
    public async Task DownloadsAgainWhenTheKeptApkIsBroken()
    {
        await installer.InstallAsync(Context(Release("2026-10-31.0", "first")), null);
        File.WriteAllText(Path.Combine(paths.ModuleDirectory("camera"), "camera.apk"), "broken");

        await installer.InstallAsync(Context(Release("2026-10-31.0", "first")), null);

        Assert.Equal(2, server.Requests);
        Assert.Equal("first", File.ReadAllText(Path.Combine(paths.ModuleDirectory("camera"), "camera.apk")));
    }

    [Fact]
    public async Task ReportsTheSizeOfTheReusedApk()
    {
        await installer.InstallAsync(Context(Release("2026-10-31.0", "first")), null);
        var progress = new Reports();

        await installer.InstallAsync(Context(Release("2026-10-31.0", "first"), progress), null);

        Assert.Equal([new DownloadProgress(5, 5)], progress.Values);
    }

    private static Release Release(string tag, string apk) =>
        new(tag, DateTimeOffset.UnixEpoch,
            [new ReleaseAsset("camera.apk", new Uri($"https://example.com/{tag}/{apk}"), apk.Length,
                Sha256Digest.FromHash(SHA256.HashData(Encoding.UTF8.GetBytes(apk))))]);

    private InstallContext Context(Release release, IProgress<DownloadProgress>? download = null) =>
        new(release, new Dictionary<PrerequisiteId, string> { [PrerequisiteId.Adb] = "adb" }, log, download,
            TestContext.Current.CancellationToken);

    /// <summary>Serves an apk whose content is the last segment of the URL.</summary>
    private sealed class Apks : HttpMessageHandler
    {
        public int Requests { get; private set; }

        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
        {
            Requests++;
            return Task.FromResult(new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent(request.RequestUri!.Segments[^1]),
            });
        }
    }

    /// <summary>adb with one phone connected.</summary>
    private sealed class Phone : ICommandRunner
    {
        /// <summary>Whether USB debugging is allowed. adb lists the phone as "unauthorized" until it is.</summary>
        public bool Authorized { get; set; } = true;

        public string InstallOutput { get; set; } = "Success";

        public Task<CommandResult> RunAsync(
            string fileName, IReadOnlyList<string> arguments, CommandOptions? options = null,
            CancellationToken cancellationToken = default) =>
            Task.FromResult(arguments switch
            {
                ["devices"] => new CommandResult(0, $"List of devices attached\n{Serial}\t{(Authorized ? "device" : "unauthorized")}\n"),
                ["-s", Serial, "install", ..] => new CommandResult(InstallOutput == "Success" ? 0 : 1, InstallOutput),
                _ => throw new InvalidOperationException($"unexpected adb {string.Join(' ', arguments)}"),
            });
    }

    private sealed class Messages : IProgress<string>
    {
        public List<string> Values { get; } = [];

        public void Report(string value) => Values.Add(value);
    }

    private sealed class Reports : IProgress<DownloadProgress>
    {
        public List<DownloadProgress> Values { get; } = [];

        public void Report(DownloadProgress value) => Values.Add(value);
    }
}
