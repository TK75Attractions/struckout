using System.Net;
using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.Core.Tests;

public sealed class DownloadTests : IDisposable
{
    // sha256("hello")
    private const string HelloSha256 = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

    private static readonly Uri Url = new("https://example.com/asset");

    private readonly string directory = Path.Combine(Path.GetTempPath(), $"download-{Guid.NewGuid()}");

    public void Dispose()
    {
        if (Directory.Exists(directory))
        {
            Directory.Delete(directory, recursive: true);
        }
    }

    [Fact]
    public void ReadsTheDigestGitHubReports()
    {
        var digest = Sha256Digest.ParseGitHub("sha256:" + HelloSha256.ToUpperInvariant());

        Assert.NotNull(digest);
        Assert.Equal(HelloSha256, digest.Hex);
        Assert.Equal("sha256:" + HelloSha256, digest.ToString());
    }

    [Theory]
    [InlineData(null)]
    [InlineData("sha512:abcd")]
    public void NoSha256DigestIsNull(string? value)
    {
        Assert.Null(Sha256Digest.ParseGitHub(value));
    }

    [Theory]
    [InlineData("sha256:abcd")]
    [InlineData("sha256:zzf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824")]
    public void MalformedSha256DigestIsAnError(string value)
    {
        Assert.Throws<FormatException>(() => Sha256Digest.ParseGitHub(value));
    }

    [Fact]
    public async Task KeepsAFileWithTheExpectedHash()
    {
        var destination = Path.Combine(directory, "asset");

        await Downloader("hello").DownloadAsync(
            Url, destination, Sha256Digest.ParseGitHub("sha256:" + HelloSha256), null, TestContext.Current.CancellationToken);

        Assert.Equal("hello", await File.ReadAllTextAsync(destination, TestContext.Current.CancellationToken));
    }

    [Fact]
    public async Task RejectsAFileWithAnotherHash()
    {
        var destination = Path.Combine(directory, "asset");

        var error = await Assert.ThrowsAsync<LauncherException>(() => Downloader("tampered").DownloadAsync(
            Url, destination, Sha256Digest.ParseGitHub("sha256:" + HelloSha256), null, TestContext.Current.CancellationToken));

        Assert.Contains("ハッシュが一致しません", error.Message, StringComparison.Ordinal);
        Assert.False(File.Exists(destination));
        Assert.False(File.Exists(destination + ".download"));
    }

    private static HttpDownloader Downloader(string body) => new(new HttpClient(new FixedResponse(body)));

    private sealed class FixedResponse(string body) : HttpMessageHandler
    {
        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken) =>
            Task.FromResult(new HttpResponseMessage(HttpStatusCode.OK) { Content = new StringContent(body) });
    }
}
