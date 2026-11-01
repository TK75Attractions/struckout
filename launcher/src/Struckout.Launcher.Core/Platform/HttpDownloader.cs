using System.Net;
using System.Net.Http.Headers;

namespace Struckout.Launcher.Core.Platform;

public sealed record DownloadProgress(long Received, long? Total);

public sealed class HttpDownloader
{
    /// <summary>
    /// How long to wait before each retry after attempts that received nothing. An attempt that received something
    /// starts the count again, so a connection that keeps dropping in the middle of a large file (camera.apk is about
    /// 180 MB) still finishes, while a server that is down gives up after about a minute.
    /// </summary>
    private static readonly TimeSpan[] DefaultRetryDelays =
    [
        TimeSpan.FromSeconds(2),
        TimeSpan.FromSeconds(5),
        TimeSpan.FromSeconds(10),
        TimeSpan.FromSeconds(20),
        TimeSpan.FromSeconds(30),
    ];

    private readonly HttpClient http;
    private readonly IReadOnlyList<TimeSpan> retryDelays;

    public HttpDownloader(HttpClient http)
        : this(http, DefaultRetryDelays)
    {
    }

    /// <param name="retryDelays">The waits before the retries in a row; its length is how many retries there are.</param>
    internal HttpDownloader(HttpClient http, IReadOnlyList<TimeSpan> retryDelays)
    {
        this.http = http;
        this.retryDelays = retryDelays;
    }

    /// <summary>
    /// Downloads <paramref name="url"/> to <paramref name="destination"/>. The file appears only when it's complete
    /// and, if <paramref name="expected"/> is given, has that hash.
    /// </summary>
    /// <remarks>
    /// When the connection drops, the download is retried. With a known hash, it continues from what it already has
    /// with an HTTP <c>Range</c> request, and what it has is kept for the next call when it gives up or is cancelled.
    /// The hash is checked once on the whole file, so a wrongly joined file is never kept. Without a hash, a joined
    /// file could not be checked, so it starts over every time.
    /// </remarks>
    /// <param name="expected">
    /// The hash the file must have. Null only for sources that publish no hash, which then rely on HTTPS alone.
    /// </param>
    /// <param name="log">Where to say that the download is retried.</param>
    /// <exception cref="LauncherException">The download failed or the file has another hash.</exception>
    public async Task DownloadAsync(
        Uri url,
        string destination,
        Sha256Digest? expected,
        IProgress<DownloadProgress>? progress,
        IProgress<string>? log,
        CancellationToken cancellationToken)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(Path.GetFullPath(destination))!);
        var partial = PartialPath(destination, expected);
        var resumable = expected is not null;
        DeletePartials(destination, keep: partial);
        if (!resumable)
        {
            File.Delete(partial);
        }

        try
        {
            var failures = 0;
            while (await DownloadOnceAsync(url, partial, resumable, progress, cancellationToken) is { } interruption)
            {
                failures = interruption.Progressed ? 1 : failures + 1;
                if (failures > retryDelays.Count)
                {
                    throw new LauncherException(
                        $"{url} をダウンロードできません: {interruption.Reason}"
                        + (resumable ? "。もう一度インストールすると、続きからダウンロードします" : ""));
                }

                var delay = retryDelays[failures - 1];
                log?.Report(
                    $"{url} のダウンロードが中断しました ({interruption.Reason})。"
                    + $"{delay.TotalSeconds} 秒後に{(resumable ? "続きから" : "最初から")}やり直します"
                    + $" ({failures}/{retryDelays.Count})");
                await Task.Delay(delay, cancellationToken);
            }

            if (expected is not null)
            {
                var actual = await Sha256Digest.OfFileAsync(partial, cancellationToken);
                if (actual != expected)
                {
                    File.Delete(partial);
                    throw new LauncherException(
                        $"{url} のハッシュが一致しません ({expected} のはずが {actual})。壊れているか、途中で書き換えられています");
                }
            }

            File.Move(partial, destination, overwrite: true);
        }
        catch (Exception e) when (!resumable && e is LauncherException or OperationCanceledException)
        {
            File.Delete(partial);
            throw;
        }
    }

    /// <summary>
    /// Where the unfinished download is kept. The name has the hash so that what is left of another version of the
    /// file is never continued.
    /// </summary>
    internal static string PartialPath(string destination, Sha256Digest? expected) =>
        expected is null ? destination + ".download" : $"{destination}.{expected.Hex}.download";

    /// <summary>Deletes what is left of unfinished downloads to <paramref name="destination"/>.</summary>
    /// <param name="keep">The one to continue, if any.</param>
    internal static void DeletePartials(string destination, string? keep = null)
    {
        var directory = Path.GetDirectoryName(Path.GetFullPath(destination))!;
        if (!Directory.Exists(directory))
        {
            return;
        }

        var name = Path.GetFileName(destination);
        foreach (var file in Directory.EnumerateFiles(directory))
        {
            var fileName = Path.GetFileName(file);
            if (fileName.StartsWith(name + ".", StringComparison.Ordinal)
                && fileName.EndsWith(".download", StringComparison.Ordinal)
                && (keep is null || Path.GetFullPath(file) != Path.GetFullPath(keep)))
            {
                File.Delete(file);
            }
        }
    }

    /// <summary>Makes one request, continuing <paramref name="partial"/> when <paramref name="resumable"/>.</summary>
    /// <returns>Why it stopped before the end, or null when <paramref name="partial"/> is complete.</returns>
    /// <exception cref="LauncherException">The server refused, and retrying won't help.</exception>
    private async Task<Interruption?> DownloadOnceAsync(
        Uri url, string partial, bool resumable, IProgress<DownloadProgress>? progress, CancellationToken cancellationToken)
    {
        var offset = resumable && File.Exists(partial) ? new FileInfo(partial).Length : 0;
        using var request = new HttpRequestMessage(HttpMethod.Get, url);
        if (offset > 0)
        {
            request.Headers.Range = new RangeHeaderValue(offset, null);
        }

        HttpResponseMessage response;
        try
        {
            response = await http.SendAsync(request, HttpCompletionOption.ResponseHeadersRead, cancellationToken);
        }
        catch (HttpRequestException e)
        {
            return new Interruption(e.Message, Progressed: false);
        }
        catch (OperationCanceledException) when (!cancellationToken.IsCancellationRequested)
        {
            // HttpClient.Timeout ran out before the headers came.
            return new Interruption("サーバーが応答しません", Progressed: false);
        }

        using (response)
        {
            long? total;
            switch (response.StatusCode)
            {
                case HttpStatusCode.PartialContent when offset > 0:
                    // https://httpwg.org/specs/rfc9110.html#field.content-range
                    var range = response.Content.Headers.ContentRange;
                    if (range is not { Unit: "bytes", HasRange: true } || range.From != offset)
                    {
                        File.Delete(partial);
                        return new Interruption($"サーバーが {offset} バイト目からではない範囲を返しました", Progressed: false);
                    }

                    total = range.Length;
                    break;
                case HttpStatusCode.OK:
                    // The server ignored the Range header, if any, and sends the whole file.
                    offset = 0;
                    total = response.Content.Headers.ContentLength;
                    break;
                case HttpStatusCode.RequestedRangeNotSatisfiable when offset > 0:
                    // We have no less than the whole file. If it's exactly the whole file, the hash tells whether
                    // it's the right one.
                    if (response.Content.Headers.ContentRange?.Length == offset)
                    {
                        return null;
                    }

                    File.Delete(partial);
                    return new Interruption("途中までのファイルが、サーバーのファイルより大きくなっています", Progressed: false);
                case >= HttpStatusCode.InternalServerError or HttpStatusCode.RequestTimeout or HttpStatusCode.TooManyRequests:
                    return new Interruption($"HTTP {(int)response.StatusCode} {response.ReasonPhrase}", Progressed: false);
                default:
                    throw new LauncherException(
                        $"{url} をダウンロードできません: HTTP {(int)response.StatusCode} {response.ReasonPhrase}");
            }

            var received = offset;
            progress?.Report(new DownloadProgress(received, total));
            await using var file = new FileStream(partial, offset > 0 ? FileMode.Append : FileMode.Create, FileAccess.Write);
            await using var source = await response.Content.ReadAsStreamAsync(cancellationToken);
            var buffer = new byte[81920];
            while (true)
            {
                int read;
                // Only the reads from the connection are retried: a full disk is not going to get better.
                try
                {
                    read = await source.ReadAsync(buffer, cancellationToken);
                }
                catch (Exception e) when (e is HttpRequestException or IOException)
                {
                    return new Interruption(e.Message, Progressed: received > offset);
                }

                if (read == 0)
                {
                    break;
                }

                await file.WriteAsync(buffer.AsMemory(0, read), cancellationToken);
                received += read;
                progress?.Report(new DownloadProgress(received, total));
            }

            return received < total
                ? new Interruption("サーバーが途中で送るのをやめました", Progressed: received > offset)
                : null;
        }
    }

    /// <param name="Progressed">Whether anything was received before it stopped.</param>
    private sealed record Interruption(string Reason, bool Progressed);
}
