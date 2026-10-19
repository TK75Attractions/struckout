using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.Core.Tests;

public class CombinedDownloadProgressTests
{
    [Fact]
    public void ReportsTheSumOfTheDownloads()
    {
        var reported = new List<DownloadProgress>();
        var combined = new CombinedDownloadProgress(new Recorder(reported));
        var projector = combined.Add()!;
        var ballTracker = combined.Add()!;

        projector.Report(new DownloadProgress(10, 100));
        ballTracker.Report(new DownloadProgress(5, 20));
        projector.Report(new DownloadProgress(60, 100));

        Assert.Equal(
            [new DownloadProgress(10, null), new DownloadProgress(15, 120), new DownloadProgress(65, 120)],
            reported);
    }

    [Fact]
    public void NothingToReportTo()
    {
        Assert.Null(new CombinedDownloadProgress(null).Add());
    }

    private sealed class Recorder(List<DownloadProgress> reported) : IProgress<DownloadProgress>
    {
        public void Report(DownloadProgress value) => reported.Add(value);
    }
}
