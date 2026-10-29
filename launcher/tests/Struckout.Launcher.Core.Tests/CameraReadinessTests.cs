using Google.Protobuf.WellKnownTypes;
using Struckout.Launcher.Core.Launch;
using Struckout.Launcher.Core.Modules;
using Tk75Attractions.Struckout.BallTracker.V1;

namespace Struckout.Launcher.Core.Tests;

public sealed class CameraReadinessTests
{
    private static readonly DateTimeOffset Now = new(2026, 10, 30, 14, 3, 34, TimeSpan.Zero);

    private static CameraObservation Ready(string name) => new(
        new CameraEntry(name + "-serial", name, name, "network", 1, [], null), true, ScreenState.Awake, true,
        new CameraStatus { Connected = true, LastFrameAt = Timestamp.FromDateTimeOffset(Now.AddSeconds(-1)), Fps = 120 });

    [Fact]
    public void RequiresEveryCameraAndAllowsUnknownScreenState()
    {
        var result = CameraReadiness.Evaluate([Ready("左カメラ"), Ready("右カメラ") with { Screen = ScreenState.Unknown }], Now, true);
        Assert.Equal(HealthKind.Up, result.Kind);
    }

    [Theory]
    [InlineData("usb", "USB でつながっていません")]
    [InlineData("pid-only", "映像が ball_tracker に届いていません")]
    [InlineData("no-frame", "映像が ball_tracker に届いていません")]
    [InlineData("no-pid", "アプリが動いていません")]
    [InlineData("locked", "ロックされています")]
    [InlineData("asleep", "スリープしています")]
    [InlineData("stale", "12 秒前")]
    [InlineData("disconnected", "切断されました")]
    public void ReportsTheUnreadyCamera(string problem, string message)
    {
        var right = Ready("右カメラ");
        right = problem switch
        {
            "usb" => right with { AdbConnected = false },
            "pid-only" => right with { Status = null },
            "no-frame" => right with { Status = new CameraStatus { Connected = true } },
            "no-pid" => right with { HasPid = false },
            "locked" => right with { Screen = ScreenState.Locked },
            "asleep" => right with { Screen = ScreenState.Asleep },
            "stale" => right with { Status = new CameraStatus { Connected = true, Fps = 120, LastFrameAt = Timestamp.FromDateTimeOffset(Now.AddSeconds(-12)) } },
            "disconnected" => right with { Status = new CameraStatus { DisconnectedAt = Timestamp.FromDateTimeOffset(Now.AddSeconds(-12)) } },
            _ => throw new ArgumentException(problem),
        };
        var result = CameraReadiness.Evaluate([Ready("左カメラ"), right], Now, true);
        Assert.Equal(HealthKind.Down, result.Kind);
        Assert.Contains("右カメラ", result.Message, StringComparison.Ordinal);
        Assert.Contains(message, result.Message, StringComparison.Ordinal);
        Assert.DoesNotContain("120", result.Message, StringComparison.Ordinal);
    }

    [Fact]
    public void RpcFailureDoesNotReusePreviousStatus()
    {
        var result = CameraReadiness.Evaluate([Ready("左カメラ"), Ready("右カメラ")], Now, false);
        Assert.Equal(HealthKind.Down, result.Kind);
        Assert.Contains("ball_tracker からカメラの状態を取れません", result.Message, StringComparison.Ordinal);
    }

    [Theory]
    [InlineData(3, HealthKind.Up)]
    [InlineData(3.001, HealthKind.Down)]
    [InlineData(-1, HealthKind.Down)]
    public void ChecksTheFreshnessBoundary(double seconds, HealthKind expected)
    {
        var camera = Ready("左カメラ");
        camera.Status!.LastFrameAt = Timestamp.FromDateTimeOffset(Now.AddSeconds(-seconds));
        Assert.Equal(expected, CameraReadiness.Evaluate([camera], Now, true).Kind);
    }

    [Fact]
    public void ReportsAllMissingCamerasAndRejectsEmptyConfiguration()
    {
        var result = CameraReadiness.Evaluate([Ready("左カメラ") with { HasPid = false }, Ready("右カメラ") with { AdbConnected = false }], Now, true);
        Assert.Contains("左カメラ", result.Message, StringComparison.Ordinal);
        Assert.Contains("右カメラ", result.Message, StringComparison.Ordinal);
        Assert.Equal(HealthKind.Down, CameraReadiness.Evaluate([], Now, true).Kind);
    }

    [Theory]
    // Representative dumpsys field layouts used by Android's power and window services.
    [InlineData("POWER MANAGER (dumpsys power)\n  mWakefulness=Awake\n  mWakefulnessChanging=false", "mShowingLockscreen=false\nmDreamingLockscreen=false", "Awake")]
    [InlineData("mWakefulness=Awake", "mShowingLockscreen=true", "Locked")]
    [InlineData("mWakefulness=Asleep", "mShowingLockscreen=false", "Asleep")]
    [InlineData("mWakefulness=Dozing", "", "Asleep")]
    [InlineData("mWakefulness=Awake", "  KeyguardServiceDelegate\n    showing=true", "Locked")]
    [InlineData("mWakefulness=Awake", "  KeyguardServiceDelegate:\n    showing=true\n    inputRestricted=true", "Locked")]
    [InlineData("mWakefulness=Awake", "  KeyguardServiceDelegate:\r\n    showing=false\r\n    inputRestricted=false", "Awake")]
    [InlineData("mWakefulness=Awake", "isStatusBarKeyguard=true", "Locked")]
    [InlineData("mWakefulness=Awake", "mKeyguardShowing=true", "Locked")]
    [InlineData("mWakefulness=Awake", "OtherWindow: showing=true", "Unknown")]
    [InlineData("", "", "Unknown")]
    [InlineData("Permission Denial", "unknown format", "Unknown")]
    public void ParsesScreenStateWithoutGuessing(string power, string window, string expected) =>
        Assert.Equal(expected, Adb.ParseScreenState(power, window).ToString());
}
