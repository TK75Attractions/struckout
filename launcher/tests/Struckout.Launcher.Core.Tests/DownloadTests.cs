using System.Net;
using System.Net.Http.Headers;
using System.Text;
using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.Core.Tests;

public sealed class DownloadTests : IDisposable
{
    // sha256("hello")
    private const string HelloSha256 = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

    // The hash of another version of the file
    private static readonly Sha256Digest Other = Sha256Digest.ParseGitHub("sha256:" + new string('a', 64))!;

    private static readonly Uri Url = new("https://example.com/asset");

    private static readonly Sha256Digest Hello = Sha256Digest.ParseGitHub("sha256:" + HelloSha256)!;

    private readonly string directory = Path.Combine(Path.GetTempPath(), $"download-{Guid.NewGuid()}");

    private string Destination => Path.Combine(directory, "asset");

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
        var server = new Server(_ => Whole("hello"));

        await Download(server, Hello);

        Assert.Equal("hello", ReadDestination());
        Assert.Equal([Destination], Directory.GetFiles(directory));
    }

    [Fact]
    public async Task RejectsAFileWithAnotherHash()
    {
        var server = new Server(_ => Whole("tampered"));

        var error = await Assert.ThrowsAsync<LauncherException>(() => Download(server, Hello));

        Assert.Contains("ハッシュが一致しません", error.Message, StringComparison.Ordinal);
        Assert.Empty(Directory.GetFiles(directory));
    }

    [Fact]
    public async Task ContinuesFromWhereTheConnectionDropped()
    {
        var server = new Server(_ => Cut("hel", length: 5), request => Part(request, "hello"));
        var log = new Messages();

        await Download(server, Hello, log);

        Assert.Equal("hello", ReadDestination());
        Assert.Equal([null, "bytes=3-"], server.Ranges);
        Assert.Contains(log.Values, m => m.Contains("続きからやり直します (1/1)", StringComparison.Ordinal));
    }

    [Fact]
    public async Task KeepsWhatItHasWhenItGivesUpSoThatTheNextInstallContinues()
    {
        var error = await Assert.ThrowsAsync<LauncherException>(() =>
            Download(new Server(_ => Cut("hel", length: 5)), Hello, retries: 0));
        Assert.Contains("続きからダウンロードします", error.Message, StringComparison.Ordinal);
        Assert.False(File.Exists(Destination));

        var server = new Server(request => Part(request, "hello"));
        await Download(server, Hello);

        Assert.Equal("hello", ReadDestination());
        Assert.Equal(["bytes=3-"], server.Ranges);
    }

    [Fact]
    public async Task KeepsGoingWhileEachAttemptReceivesSomething()
    {
        // One retry in a row is allowed, but every attempt gets one more byte.
        var server = new Server(Enumerable.Range(0, 4)
            .Select(i => (Func<HttpRequestMessage, HttpResponseMessage>)(request =>
                i == 0 ? Cut("h", length: 5) : CutPart(request, "hello", 1)))
            .Append(request => Part(request, "hello"))
            .ToArray());

        await Download(server, Hello, retries: 1);

        Assert.Equal("hello", ReadDestination());
        Assert.Equal([null, "bytes=1-", "bytes=2-", "bytes=3-", "bytes=4-"], server.Ranges);
    }

    [Fact]
    public async Task GivesUpAfterTheRetriesWhenNothingArrives()
    {
        var server = new Server(Enumerable.Repeat<Func<HttpRequestMessage, HttpResponseMessage>>(
            _ => throw new HttpRequestException("connection refused"), 3).ToArray());

        var error = await Assert.ThrowsAsync<LauncherException>(() => Download(server, Hello, retries: 2));

        Assert.Contains("connection refused", error.Message, StringComparison.Ordinal);
        Assert.Equal(3, server.Ranges.Count);
    }

    [Fact]
    public async Task RetriesServerErrors()
    {
        var server = new Server(_ => new HttpResponseMessage(HttpStatusCode.ServiceUnavailable), _ => Whole("hello"));

        await Download(server, Hello);

        Assert.Equal("hello", ReadDestination());
    }

    [Fact]
    public async Task DoesNotRetryWhatTheServerRefuses()
    {
        var server = new Server(_ => new HttpResponseMessage(HttpStatusCode.NotFound));

        var error = await Assert.ThrowsAsync<LauncherException>(() => Download(server, Hello, retries: 3));

        Assert.Contains("404", error.Message, StringComparison.Ordinal);
        Assert.Single(server.Ranges);
    }

    [Fact]
    public async Task StartsOverWhenTheServerIgnoresTheRange()
    {
        WritePartial(Hello, "hel");
        var server = new Server(_ => Whole("hello"));

        await Download(server, Hello);

        Assert.Equal("hello", ReadDestination());
        Assert.Equal(["bytes=3-"], server.Ranges);
    }

    [Fact]
    public async Task StartsOverWhenTheServerSendsAnotherRange()
    {
        WritePartial(Hello, "hel");
        var server = new Server(_ => Range("hello", from: 0), _ => Whole("hello"));

        await Download(server, Hello);

        Assert.Equal("hello", ReadDestination());
        Assert.Equal(["bytes=3-", null], server.Ranges);
    }

    [Fact]
    public async Task ChecksWhatItHasWhenItIsAlreadyComplete()
    {
        WritePartial(Hello, "hello");
        var server = new Server(_ => NotSatisfiable(length: 5));

        await Download(server, Hello);

        Assert.Equal("hello", ReadDestination());
        Assert.Equal(["bytes=5-"], server.Ranges);
    }

    [Fact]
    public async Task NeverContinuesAnotherVersionOfTheFile()
    {
        WritePartial(Other, "oth");
        Directory.CreateDirectory(directory);
        File.WriteAllText(Destination + ".download", "hel");
        var server = new Server(_ => Whole("hello"));

        await Download(server, Hello);

        Assert.Equal("hello", ReadDestination());
        Assert.Equal([null], server.Ranges);
        Assert.Equal([Destination], Directory.GetFiles(directory));
    }

    [Fact]
    public async Task StartsOverWithoutAHash()
    {
        Directory.CreateDirectory(directory);
        File.WriteAllText(HttpDownloader.PartialPath(Destination, null), "hel");
        var server = new Server(_ => Cut("he", length: 5), _ => Whole("hello"));

        await Download(server, expected: null);

        Assert.Equal("hello", ReadDestination());
        Assert.Equal([null, null], server.Ranges);
    }

    private Task Download(Server server, Sha256Digest? expected, Messages? log = null, int retries = 1) =>
        new HttpDownloader(new HttpClient(server), Enumerable.Repeat(TimeSpan.Zero, retries).ToList())
            .DownloadAsync(Url, Destination, expected, null, log, TestContext.Current.CancellationToken);

    private string ReadDestination() => File.ReadAllText(Destination);

    private void WritePartial(Sha256Digest digest, string content)
    {
        Directory.CreateDirectory(directory);
        File.WriteAllText(HttpDownloader.PartialPath(Destination, digest), content);
    }

    private static HttpResponseMessage Whole(string body) =>
        new(HttpStatusCode.OK) { Content = new StringContent(body) };

    /// <summary>Sends <paramref name="body"/> of a file of <paramref name="length"/> bytes, then drops the connection.</summary>
    private static HttpResponseMessage Cut(string body, long length)
    {
        var content = new StreamContent(new DroppingStream(Encoding.UTF8.GetBytes(body)));
        content.Headers.ContentLength = length;
        return new HttpResponseMessage(HttpStatusCode.OK) { Content = content };
    }

    /// <summary>Sends what the request asks for of <paramref name="file"/>.</summary>
    private static HttpResponseMessage Part(HttpRequestMessage request, string file) =>
        Range(file, from: request.Headers.Range!.Ranges.Single().From!.Value);

    /// <summary>Sends <paramref name="count"/> bytes of what the request asks for, then drops the connection.</summary>
    private static HttpResponseMessage CutPart(HttpRequestMessage request, string file, int count)
    {
        var from = (int)request.Headers.Range!.Ranges.Single().From!.Value;
        var content = new StreamContent(new DroppingStream(Encoding.UTF8.GetBytes(file[from..(from + count)])));
        content.Headers.ContentRange = new ContentRangeHeaderValue(from, file.Length - 1, file.Length);
        return new HttpResponseMessage(HttpStatusCode.PartialContent) { Content = content };
    }

    private static HttpResponseMessage Range(string file, long from)
    {
        var content = new StringContent(file[(int)from..]);
        content.Headers.ContentRange = new ContentRangeHeaderValue(from, file.Length - 1, file.Length);
        return new HttpResponseMessage(HttpStatusCode.PartialContent) { Content = content };
    }

    private static HttpResponseMessage NotSatisfiable(long length)
    {
        var content = new ByteArrayContent([]);
        content.Headers.ContentRange = new ContentRangeHeaderValue(length);
        return new HttpResponseMessage(HttpStatusCode.RequestedRangeNotSatisfiable) { Content = content };
    }

    /// <summary>Answers the requests in order, and records the Range header of each.</summary>
    private sealed class Server(params Func<HttpRequestMessage, HttpResponseMessage>[] responses) : HttpMessageHandler
    {
        public List<string?> Ranges { get; } = [];

        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
        {
            Ranges.Add(request.Headers.Range?.ToString());
            Assert.True(Ranges.Count <= responses.Length, "more requests than expected");
            return Task.FromResult(responses[Ranges.Count - 1](request));
        }
    }

    /// <summary>A response body whose connection drops after <c>data</c>.</summary>
    private sealed class DroppingStream(byte[] data) : Stream
    {
        private int position;

        public override bool CanRead => true;

        public override bool CanSeek => false;

        public override bool CanWrite => false;

        public override long Length => throw new NotSupportedException();

        public override long Position
        {
            get => throw new NotSupportedException();
            set => throw new NotSupportedException();
        }

        public override int Read(byte[] buffer, int offset, int count) => Read(buffer.AsSpan(offset, count));

        public override int Read(Span<byte> buffer)
        {
            if (position == data.Length)
            {
                throw new IOException("An existing connection was forcibly closed by the remote host");
            }

            var read = Math.Min(buffer.Length, data.Length - position);
            data.AsSpan(position, read).CopyTo(buffer);
            position += read;
            return read;
        }

        public override ValueTask<int> ReadAsync(Memory<byte> buffer, CancellationToken cancellationToken = default) =>
            ValueTask.FromResult(Read(buffer.Span));

        public override Task<int> ReadAsync(byte[] buffer, int offset, int count, CancellationToken cancellationToken) =>
            Task.FromResult(Read(buffer.AsSpan(offset, count)));

        public override void Flush()
        {
        }

        public override long Seek(long offset, SeekOrigin origin) => throw new NotSupportedException();

        public override void SetLength(long value) => throw new NotSupportedException();

        public override void Write(byte[] buffer, int offset, int count) => throw new NotSupportedException();
    }

    private sealed class Messages : IProgress<string>
    {
        public List<string> Values { get; } = [];

        public void Report(string value) => Values.Add(value);
    }
}
