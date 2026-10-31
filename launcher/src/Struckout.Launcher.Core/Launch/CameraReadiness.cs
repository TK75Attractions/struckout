using Struckout.Launcher.Core.Modules;
using Tk75Attractions.Struckout.BallTracker.V1;

namespace Struckout.Launcher.Core.Launch;

internal sealed record CameraObservation(CameraEntry Camera, bool AdbConnected, ScreenState Screen, bool HasPid, CameraStatus? Status);

/// <summary>Requires every configured phone to deliver recent frames, rather than just have a running process.</summary>
internal static class CameraReadiness
{
    /// <summary>Frames older than this mean the video has stopped, even if the connection is still open.</summary>
    internal static readonly TimeSpan MaxFrameAge = TimeSpan.FromSeconds(3);

    /// <summary>Whether ball_tracker received a frame from the camera recently.</summary>
    internal static bool IsReceiving(CameraStatus? status, DateTimeOffset now)
    {
        if (status is not { Connected: true, LastFrameAt: not null })
        {
            return false;
        }

        var age = now - status.LastFrameAt.ToDateTimeOffset();
        return age >= TimeSpan.Zero && age <= MaxFrameAge;
    }

    internal static Health Evaluate(IReadOnlyList<CameraObservation> cameras, DateTimeOffset now, bool rpcSucceeded)
    {
        var reasons = new List<string>();
        if (!rpcSucceeded)
        {
            reasons.Add("ball_tracker からカメラの状態を取れません。ball_tracker の起動と接続を確かめてください");
        }

        foreach (var observation in cameras)
        {
            if (Reason(observation, now, rpcSucceeded) is { } reason)
            {
                reasons.Add(reason);
            }
        }

        if (cameras.Count == 0)
        {
            reasons.Add("cameras.json にカメラがありません");
        }

        return reasons.Count > 0
            ? Health.Down(string.Join("\n", reasons))
            : Health.Up($"{string.Join("、", cameras.Select(c => c.Camera.Name))} の映像を受信しています");
    }

    private static string? Reason(CameraObservation observation, DateTimeOffset now, bool rpcSucceeded)
    {
        var (camera, connected, screen, hasPid, status) = observation;
        var name = camera.Name;
        if (!connected)
        {
            return $"{name} ({camera.Serial}) が USB でつながっていません。USB でつなぎ、スマホの『USB デバッグを許可』を確かめてください";
        }

        if (screen == ScreenState.Locked)
        {
            return $"{name}の画面がロックされています。ロックを解除してください";
        }

        if (screen == ScreenState.Asleep)
        {
            return $"{name}の画面がスリープしています。画面を点けてロックを解除してください";
        }

        if (!hasPid)
        {
            return $"{name}のアプリが動いていません。端末でカメラアプリを開いてください";
        }

        if (!rpcSucceeded || IsReceiving(status, now))
        {
            return null;
        }

        if (status is { Connected: false, DisconnectedAt: not null })
        {
            return $"{name}が ball_tracker から切断されました ({status.DisconnectedAt.ToDateTimeOffset().ToLocalTime():HH:mm:ss})。USB 接続とアプリを確かめてください";
        }

        if (status is not { Connected: true, LastFrameAt: not null })
        {
            return $"{name}の映像が ball_tracker に届いていません。アプリの撮影画面と接続設定を確かめてください";
        }

        // Old values such as fps are not shown, so a stopped camera doesn't look like it works (#327).
        var last = status.LastFrameAt.ToDateTimeOffset();
        var seconds = Math.Max(0, Math.Floor((now - last).TotalSeconds));
        return $"{name}の映像が止まっています (最後に受信: {last.ToLocalTime():HH:mm:ss}、{seconds} 秒前)。撮影画面と USB 接続を確かめてください";
    }
}
