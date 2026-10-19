namespace Struckout.Launcher.Core.Platform;

/// <summary>Reports several downloads running at once as one total, for a single progress bar.</summary>
internal sealed class CombinedDownloadProgress(IProgress<DownloadProgress>? total)
{
    private readonly List<DownloadProgress> parts = [];

    /// <summary>The progress of one more download.</summary>
    public IProgress<DownloadProgress>? Add()
    {
        if (total is null)
        {
            return null;
        }

        int index;
        lock (parts)
        {
            index = parts.Count;
            parts.Add(new DownloadProgress(0, null));
        }

        return new Part(this, index);
    }

    private void Report(int index, DownloadProgress progress)
    {
        DownloadProgress sum;
        lock (parts)
        {
            parts[index] = progress;
            // The total is unknown until every download knows its size.
            sum = new DownloadProgress(
                parts.Sum(p => p.Received),
                parts.All(p => p.Total is not null) ? parts.Sum(p => p.Total!.Value) : null);
        }

        total!.Report(sum);
    }

    private sealed class Part(CombinedDownloadProgress owner, int index) : IProgress<DownloadProgress>
    {
        public void Report(DownloadProgress value) => owner.Report(index, value);
    }
}
