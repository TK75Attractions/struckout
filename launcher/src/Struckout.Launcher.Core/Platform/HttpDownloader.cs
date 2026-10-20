using System.Security.Cryptography;

namespace Struckout.Launcher.Core.Platform;

public sealed record DownloadProgress(long Received, long? Total);

public sealed class HttpDownloader(HttpClient http)
{
    /// <summary>
    /// Downloads <paramref name="url"/> to <paramref name="destination"/>. The file appears only when it's complete
    /// and, if <paramref name="expected"/> is given, has that hash.
    /// </summary>
    /// <param name="expected">
    /// The hash the file must have. Null only for sources that publish no hash, which then rely on HTTPS alone.
    /// </param>
    /// <exception cref="LauncherException">The download failed or the file has another hash.</exception>
    public async Task DownloadAsync(
        Uri url,
        string destination,
        Sha256Digest? expected,
        IProgress<DownloadProgress>? progress,
        CancellationToken cancellationToken)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(Path.GetFullPath(destination))!);
        var partial = destination + ".download";
        try
        {
            using var response = await http.GetAsync(url, HttpCompletionOption.ResponseHeadersRead, cancellationToken);
            response.EnsureSuccessStatusCode();
            var total = response.Content.Headers.ContentLength;
            using var hash = IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
            await using (var source = await response.Content.ReadAsStreamAsync(cancellationToken))
            await using (var file = File.Create(partial))
            {
                var buffer = new byte[81920];
                long received = 0;
                int read;
                while ((read = await source.ReadAsync(buffer, cancellationToken)) > 0)
                {
                    await file.WriteAsync(buffer.AsMemory(0, read), cancellationToken);
                    hash.AppendData(buffer, 0, read);
                    received += read;
                    progress?.Report(new DownloadProgress(received, total));
                }
            }

            var actual = Sha256Digest.FromHash(hash.GetHashAndReset());
            if (expected is not null && actual != expected)
            {
                File.Delete(partial);
                throw new LauncherException(
                    $"{url} のハッシュが一致しません ({expected} のはずが {actual})。壊れているか、途中で書き換えられています");
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
