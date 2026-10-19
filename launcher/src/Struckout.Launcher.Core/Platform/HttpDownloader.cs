namespace Struckout.Launcher.Core.Platform;

public sealed record DownloadProgress(long Received, long? Total);

public sealed class HttpDownloader(HttpClient http)
{
    /// <summary>
    /// Downloads <paramref name="url"/> to <paramref name="destination"/>. The file appears only when it's complete.
    /// </summary>
    /// <exception cref="LauncherException">The download failed.</exception>
    public async Task DownloadAsync(
        Uri url, string destination, IProgress<DownloadProgress>? progress, CancellationToken cancellationToken)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(Path.GetFullPath(destination))!);
        var partial = destination + ".download";
        try
        {
            using var response = await http.GetAsync(url, HttpCompletionOption.ResponseHeadersRead, cancellationToken);
            response.EnsureSuccessStatusCode();
            var total = response.Content.Headers.ContentLength;
            await using (var source = await response.Content.ReadAsStreamAsync(cancellationToken))
            await using (var file = File.Create(partial))
            {
                var buffer = new byte[81920];
                long received = 0;
                int read;
                while ((read = await source.ReadAsync(buffer, cancellationToken)) > 0)
                {
                    await file.WriteAsync(buffer.AsMemory(0, read), cancellationToken);
                    received += read;
                    progress?.Report(new DownloadProgress(received, total));
                }
            }

            File.Move(partial, destination, overwrite: true);
        }
        catch (HttpRequestException e)
        {
            File.Delete(partial);
            throw new LauncherException($"{url} をダウンロードできません: {e.Message}", e);
        }
        catch (OperationCanceledException)
        {
            File.Delete(partial);
            throw;
        }
    }
}
