using System.IO.Compression;
using System.Net;
using System.Security.Cryptography;
using Struckout.Launcher.Core.GitHub;
using Struckout.Launcher.Core.Install;
using Struckout.Launcher.Core.Launch;
using Struckout.Launcher.Core.Modules;
using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.Core.Tests;

public sealed class ProjectorInstallerTests : IDisposable
{
    private readonly string directory = Path.Combine(Path.GetTempPath(), $"projector-{Guid.NewGuid()}");

    public void Dispose()
    {
        if (Directory.Exists(directory))
        {
            Directory.Delete(directory, recursive: true);
        }
    }

    [Theory]
    [InlineData(OperatingSystemKind.Windows)]
    [InlineData(OperatingSystemKind.Linux)]
    [InlineData(OperatingSystemKind.MacOS)]
    public async Task InstallsAndValidatesEveryPlatform(OperatingSystemKind os)
    {
        var archive = CreateArchive(os);
        var extracting = Path.Combine(directory, "player.new");

        await ProjectorFiles.ExtractAsync(archive, extracting, os, TestContext.Current.CancellationToken);
        ProjectorFiles.Replace(Path.Combine(directory, "player"));
        ProjectorFiles.ValidateForLaunch(Path.Combine(directory, "player"), os, new Progress<string>());

        Assert.True(File.Exists(Path.Combine(directory, "player", ProjectorFiles.ManifestName)));
    }

    [Theory]
    [InlineData("StandaloneWindows64/UnityPlayer.dll")]
    [InlineData("StandaloneWindows64/StandaloneWindows64_Data/globalgamemanagers")]
    [InlineData("StandaloneWindows64/StandaloneWindows64_Data/Managed/Struckout.Bootstrap.dll")]
    public async Task RejectsIncompleteZipAndKeepsExistingInstallation(string missing)
    {
        var archive = CreateArchive(OperatingSystemKind.Windows, missing);
        var bytes = await File.ReadAllBytesAsync(archive, TestContext.Current.CancellationToken);
        var paths = new AppPaths(Path.Combine(directory, "config"), Path.Combine(directory, "data"));
        var installed = paths.ModuleDirectory("projector");
        WriteFile(installed, "previous", "original");
        var release = new Release("new", DateTimeOffset.UnixEpoch,
            [new ReleaseAsset("projector-x86_64-windows.zip", new Uri("https://example.com/player.zip"), bytes.Length,
                Sha256Digest.ParseGitHub("sha256:" + Convert.ToHexString(SHA256.HashData(bytes))))]);
        var installer = new ProjectorInstaller(new HostPlatform(OperatingSystemKind.Windows, CpuArchitecture.X64), paths,
            new HttpDownloader(new HttpClient(new ArchiveResponse(bytes))));
        var context = new InstallContext(release, new Dictionary<PrerequisiteId, string>(), new Progress<string>(), null,
            TestContext.Current.CancellationToken);

        var error = await Assert.ThrowsAsync<LauncherException>(() => installer.InstallAsync(context, null));

        Assert.Contains(missing, error.Message, StringComparison.Ordinal);
        Assert.Equal("original", File.ReadAllText(Path.Combine(installed, "previous")));
        Assert.False(Directory.Exists(installed + ".old"));
    }

    [Fact]
    public async Task SuccessfulInstallRecoversAndReplacesThePreviousDirectory()
    {
        var archive = CreateArchive(OperatingSystemKind.Windows);
        var bytes = await File.ReadAllBytesAsync(archive, TestContext.Current.CancellationToken);
        var paths = new AppPaths(Path.Combine(directory, "config"), Path.Combine(directory, "data"));
        var installed = paths.ModuleDirectory("projector");
        WriteFile(installed + ".old", "previous", "original");
        var release = new Release("new", DateTimeOffset.UnixEpoch,
            [new ReleaseAsset("projector-x86_64-windows.zip", new Uri("https://example.com/player.zip"), bytes.Length,
                Sha256Digest.ParseGitHub("sha256:" + Convert.ToHexString(SHA256.HashData(bytes))))]);
        var installer = new ProjectorInstaller(new HostPlatform(OperatingSystemKind.Windows, CpuArchitecture.X64), paths,
            new HttpDownloader(new HttpClient(new ArchiveResponse(bytes))));
        var context = new InstallContext(release, new Dictionary<PrerequisiteId, string>(), new Progress<string>(), null,
            TestContext.Current.CancellationToken);

        var installation = Assert.IsType<ExecutableInstallation>(await installer.InstallAsync(context, null));

        Assert.Equal("new", installation.ReleaseTag);
        Assert.Equal(installed, installation.Directory);
        Assert.True(File.Exists(installation.Executable));
        Assert.False(File.Exists(Path.Combine(installed, "previous")));
        Assert.False(Directory.Exists(installed + ".old"));
        Assert.False(Directory.Exists(installed + ".new"));
        ProjectorFiles.ValidateForLaunch(installed, OperatingSystemKind.Windows, new Progress<string>());

        WriteFile(installed + ".old", "previous", "original");
        WriteFile(installed + ".new", "partial", "partial");
        var partialDownload = HttpDownloader.PartialPath(installed + ".zip", release.Assets[0].Digest);
        File.WriteAllText(partialDownload, "partial");
        await installer.UninstallAsync(installation, new Dictionary<PrerequisiteId, string>(), new UninstallOptions(),
            new Progress<string>(), TestContext.Current.CancellationToken);

        Assert.False(Directory.Exists(installed));
        Assert.False(Directory.Exists(installed + ".old"));
        Assert.False(Directory.Exists(installed + ".new"));
        Assert.False(File.Exists(partialDownload));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public async Task DetectsMissingOrTruncatedNonessentialFile(bool truncate)
    {
        var archive = CreateArchive(OperatingSystemKind.Linux);
        var installed = Path.Combine(directory, "player");
        await ProjectorFiles.ExtractAsync(archive, installed, OperatingSystemKind.Linux, TestContext.Current.CancellationToken);
        var file = Path.Combine(installed, "extra", "asset");
        if (truncate)
        {
            File.WriteAllText(file, "");
        }
        else
        {
            File.Delete(file);
        }

        var error = Assert.Throws<LauncherException>(() => ProjectorFiles.ValidateForLaunch(installed, OperatingSystemKind.Linux, new Progress<string>()));

        Assert.Contains("extra/asset", error.Message, StringComparison.Ordinal);
        Assert.Contains("インストール画面で projector を入れ直してください", error.Message, StringComparison.Ordinal);
    }

    [Fact]
    public async Task RefusesToStartAPlayerWithMissingFiles()
    {
        var archive = CreateArchive(OperatingSystemKind.Windows);
        var installed = Path.Combine(directory, "player");
        await ProjectorFiles.ExtractAsync(archive, installed, OperatingSystemKind.Windows, TestContext.Current.CancellationToken);
        File.Delete(Path.Combine(installed, "extra", "asset"));
        var runner = new ProcessModuleRunner(ModuleId.Projector,
            new HostPlatform(OperatingSystemKind.Windows, CpuArchitecture.X64), new ProcessCommandRunner(),
            Path.Combine(installed, ProjectorInstaller.ExecutableInArchive(OperatingSystemKind.Windows)),
            [], [], null, null, verify: log => ProjectorFiles.ValidateForLaunch(installed, OperatingSystemKind.Windows, log));

        var error = await Assert.ThrowsAsync<LauncherException>(() =>
            runner.StartAsync(new Progress<string>(), TestContext.Current.CancellationToken));

        Assert.Contains("extra/asset", error.Message, StringComparison.Ordinal);
        Assert.Equal(HealthKind.Down, (await runner.CheckAsync(TestContext.Current.CancellationToken)).Kind);
    }

    [Fact]
    public async Task ChecksEveryArchiveEntryAfterExtraction()
    {
        var archive = CreateArchive(OperatingSystemKind.Linux);
        var installed = Path.Combine(directory, "player");
        await ProjectorFiles.ExtractAsync(archive, installed, OperatingSystemKind.Linux, TestContext.Current.CancellationToken);
        using var zip = ZipFile.OpenRead(archive);
        File.WriteAllText(Path.Combine(installed, "extra", "asset"), "");
        Assert.Throws<LauncherException>(() => ProjectorFiles.ValidateArchive(zip, installed));
        File.Delete(Path.Combine(installed, "extra", "asset"));
        Assert.Throws<LauncherException>(() => ProjectorFiles.ValidateArchive(zip, installed));
        Directory.Delete(Path.Combine(installed, "empty"));
        var error = Assert.Throws<LauncherException>(() => ProjectorFiles.ValidateArchive(zip, installed));
        Assert.Contains("empty/", error.Message, StringComparison.Ordinal);
    }

    [Fact]
    public void RestoresPreviousFilesWhenSecondRenameFails()
    {
        var installed = Path.Combine(directory, "player");
        WriteFile(installed, "previous", "original");
        WriteFile(installed + ".new", "replacement", "new");

        Assert.Throws<LauncherException>(() => ProjectorFiles.Replace(installed, (source, destination) =>
        {
            if (source.EndsWith(".new", StringComparison.Ordinal))
            {
                throw new IOException("Simulated rename failure");
            }

            Directory.Move(source, destination);
        }));

        Assert.Equal("original", File.ReadAllText(Path.Combine(installed, "previous")));
        Assert.True(File.Exists(Path.Combine(installed + ".new", "replacement")));
        Assert.False(Directory.Exists(installed + ".old"));
    }

    [Fact]
    public void KeepsBackupWhenRollbackAlsoFailsAndRecoversOnNextAttempt()
    {
        var installed = Path.Combine(directory, "player");
        WriteFile(installed, "previous", "original");
        WriteFile(installed + ".new", "replacement", "new");

        Assert.Throws<LauncherException>(() => ProjectorFiles.Replace(installed, (source, destination) =>
        {
            if (source.EndsWith(".new", StringComparison.Ordinal) || source.EndsWith(".old", StringComparison.Ordinal))
            {
                throw new IOException("Simulated rename failure");
            }

            Directory.Move(source, destination);
        }));
        Assert.False(Directory.Exists(installed));
        Assert.Equal("original", File.ReadAllText(Path.Combine(installed + ".old", "previous")));

        ProjectorFiles.Recover(installed);

        Assert.Equal("original", File.ReadAllText(Path.Combine(installed, "previous")));
    }

    [Fact]
    public async Task RecoversOldDirectoryBeforeLaunch()
    {
        var archive = CreateArchive(OperatingSystemKind.Linux);
        var installed = Path.Combine(directory, "player");
        await ProjectorFiles.ExtractAsync(archive, installed + ".old", OperatingSystemKind.Linux, TestContext.Current.CancellationToken);

        ProjectorFiles.ValidateForLaunch(installed, OperatingSystemKind.Linux, new Progress<string>());

        Assert.True(File.Exists(Path.Combine(installed, ProjectorFiles.ManifestName)));
        Assert.False(Directory.Exists(installed + ".old"));
    }

    [Fact]
    public void RecoversOldDirectoryBeforeReplacingAndKeepsItOnFailure()
    {
        var installed = Path.Combine(directory, "player");
        WriteFile(installed + ".old", "previous", "original");
        WriteFile(installed + ".new", "replacement", "new");
        Assert.Throws<LauncherException>(() => ProjectorFiles.Replace(installed, (source, destination) =>
        {
            if (source.EndsWith(".new", StringComparison.Ordinal))
            {
                throw new IOException("Simulated rename failure");
            }

            Directory.Move(source, destination);
        }));
        Assert.Equal("original", File.ReadAllText(Path.Combine(installed, "previous")));
    }

    [Theory]
    [InlineData("{broken")]
    [InlineData("null")]
    [InlineData("{}")]
    public void RejectsInvalidManifest(string manifest)
    {
        WriteFile(directory, ProjectorFiles.ManifestName, manifest);

        var error = Assert.Throws<LauncherException>(() => ProjectorFiles.ValidateForLaunch(directory, OperatingSystemKind.Linux, new Progress<string>()));

        Assert.Contains("インストール画面で projector を入れ直してください", error.Message, StringComparison.Ordinal);
    }

    [Theory]
    [InlineData(OperatingSystemKind.Windows)]
    [InlineData(OperatingSystemKind.Linux)]
    [InlineData(OperatingSystemKind.MacOS)]
    public async Task ValidatesLegacyInstallationAndReportsOnce(OperatingSystemKind os)
    {
        var installed = Path.Combine(directory, "player");
        await ZipFile.ExtractToDirectoryAsync(CreateArchive(os), installed, TestContext.Current.CancellationToken);
        File.Delete(Path.Combine(installed, "extra", "asset"));
        var log = new LogRecorder();

        ProjectorFiles.ValidateForLaunch(installed, os, log);

        Assert.Contains("次の更新でファイルの一覧を記録します", Assert.Single(log.Messages), StringComparison.Ordinal);
        Assert.False(File.Exists(Path.Combine(installed, ProjectorFiles.ManifestName)));
    }

    [Theory]
    [InlineData(OperatingSystemKind.Windows)]
    [InlineData(OperatingSystemKind.Linux)]
    [InlineData(OperatingSystemKind.MacOS)]
    public async Task RejectsLegacyInstallationWhenAnyRequiredFileIsMissing(OperatingSystemKind os)
    {
        var installed = Path.Combine(directory, "player");
        await ZipFile.ExtractToDirectoryAsync(CreateArchive(os), installed, TestContext.Current.CancellationToken);
        foreach (var required in ProjectorFiles.RequiredFiles(os))
        {
            var file = Path.Combine(installed, required);
            File.Delete(file);
            var log = new LogRecorder();

            var error = Assert.Throws<LauncherException>(() => ProjectorFiles.ValidateForLaunch(installed, os, log));

            Assert.Contains(required, error.Message, StringComparison.Ordinal);
            Assert.Empty(log.Messages);
            File.WriteAllText(file, "contents");
        }
    }

    private sealed class LogRecorder : IProgress<string>
    {
        public List<string> Messages { get; } = [];

        public void Report(string value) => Messages.Add(value);
    }

    private string CreateArchive(OperatingSystemKind os, string? missing = null)
    {
        Directory.CreateDirectory(directory);
        var archive = Path.Combine(directory, "player.zip");
        using var zip = ZipFile.Open(archive, ZipArchiveMode.Create);
        zip.CreateEntry("empty/");
        string[] files = os switch
        {
            OperatingSystemKind.Windows =>
            [
                "StandaloneWindows64/StandaloneWindows64.exe",
                "StandaloneWindows64/UnityPlayer.dll",
                "StandaloneWindows64/StandaloneWindows64_Data/globalgamemanagers",
                .. ScriptAssemblies("StandaloneWindows64/StandaloneWindows64_Data"),
            ],
            OperatingSystemKind.Linux =>
            [
                "StandaloneLinux64/StandaloneLinux64.x86_64",
                "StandaloneLinux64/UnityPlayer.so",
                "StandaloneLinux64/StandaloneLinux64_Data/globalgamemanagers",
                .. ScriptAssemblies("StandaloneLinux64/StandaloneLinux64_Data"),
            ],
            OperatingSystemKind.MacOS =>
            [
                "StandaloneOSX/StandaloneOSX.app/Contents/MacOS/StruckOut",
                "StandaloneOSX/StandaloneOSX.app/Contents/Frameworks/UnityPlayer.dylib",
                "StandaloneOSX/StandaloneOSX.app/Contents/Resources/Data/globalgamemanagers",
                .. ScriptAssemblies("StandaloneOSX/StandaloneOSX.app/Contents/Resources/Data"),
            ],
            _ => throw new ArgumentOutOfRangeException(nameof(os)),
        };
        foreach (var file in files.Append("extra/asset"))
        {
            if (file != missing)
            {
                using var writer = new StreamWriter(zip.CreateEntry(file).Open());
                writer.Write("contents");
            }
        }

        return archive;
    }

    /// <summary>The script assemblies of the released players, one per asmdef in projector/Assets/Scripts.</summary>
    /// <remarks>
    /// Copied from the released zips, not from <see cref="ProjectorFiles.RequiredFiles"/>, so that the tests catch a required file
    /// the player does not ship. There is no Assembly-CSharp.dll because every runtime script is in an asmdef.
    /// </remarks>
    private static IEnumerable<string> ScriptAssemblies(string data)
    {
        string[] names = ["Struckout.Application", "Struckout.Bootstrap", "Struckout.Domain",
            "Struckout.Generated", "Struckout.Infrastructure", "Struckout.Unity"];
        return names.Select(name => $"{data}/Managed/{name}.dll");
    }

    private static void WriteFile(string root, string relative, string contents)
    {
        var file = Path.Combine(root, relative);
        Directory.CreateDirectory(Path.GetDirectoryName(file)!);
        File.WriteAllText(file, contents);
    }

    private sealed class ArchiveResponse(byte[] bytes) : HttpMessageHandler
    {
        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken) =>
            Task.FromResult(new HttpResponseMessage(HttpStatusCode.OK) { Content = new ByteArrayContent(bytes) });
    }
}
