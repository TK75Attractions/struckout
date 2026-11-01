using Struckout.Launcher.Core.Platform;
using Tk75Attractions.Struckout.BallTracker.V1;

namespace Struckout.Launcher.Core.Launch;

/// <summary>A camera whose video stopped reaching ball_tracker.</summary>
public sealed record CameraVideoLoss(uint CameraId, string Name, string Message);

/// <summary>
/// Sounds an alarm while a camera that has sent video stops sending it, so that the staff notice at once
/// a phone whose screen went off or whose cable came out during a game (#421).
/// </summary>
/// <remarks>
/// <see cref="CameraRunner"/> feeds it the camera status it already asks ball_tracker for, and a camera counts as
/// sending video by <see cref="CameraReadiness.IsReceiving"/>, the same rule as the camera's readiness.
/// Only cameras that have sent video since <see cref="Reset"/> are watched. While the modules are being started,
/// the phones are opened one by one and haven't sent anything yet, which must not sound the alarm.
/// The sound starts once when a camera stops and stops when every watched camera sends video again,
/// rather than beeping on every check.
/// </remarks>
/// <param name="createSound">
/// Called when the alarm first sounds, so that a PC whose cameras never stop doesn't look for a player.
/// </param>
public sealed class CameraVideoAlarm(Func<IAlarmSound?> createSound, IProgress<string> log)
{
    private readonly Lock gate = new();

    /// <summary>Cameras that have sent video, by <c>camera_id</c>.</summary>
    private readonly Dictionary<uint, string> watched = [];

    private readonly Lazy<IAlarmSound?> sound = new(createSound);
    private IReadOnlyList<CameraVideoLoss> losses = [];
    private DateTimeOffset? lastAnswerAt;
    private bool muted;
    private bool sounding;

    public IReadOnlyList<CameraVideoLoss> Losses
    {
        get
        {
            lock (gate)
            {
                return losses;
            }
        }
    }

    /// <summary>The cameras have stopped but <see cref="Mute"/> silenced the sound.</summary>
    public bool IsMuted
    {
        get
        {
            lock (gate)
            {
                return muted && losses.Count > 0;
            }
        }
    }

    /// <param name="statuses">
    /// ball_tracker's <c>ListCameraStatus</c>, or null when it didn't answer (it exited, or the call timed out).
    /// </param>
    /// <param name="now">
    /// Compared with <c>last_frame_at</c>, the clock of the PC running ball_tracker, which is this PC.
    /// </param>
    public void Update(IReadOnlyList<CameraStatus>? statuses, DateTimeOffset now)
    {
        lock (gate)
        {
            List<CameraVideoLoss> next;
            if (statuses is null)
            {
                // One failed call may be a timeout of a busy ball_tracker: wait as long as for a frame.
                if (lastAnswerAt is { } last && now - last <= CameraReadiness.MaxFrameAge)
                {
                    return;
                }

                next = watched
                    .Select(camera => new CameraVideoLoss(
                        camera.Key, camera.Value, $"{camera.Value}の映像を確かめられません。ball_tracker が動いているか確かめてください"))
                    .ToList();
            }
            else
            {
                lastAnswerAt = now;
                // Devices missing from cameras.json have no name and are not part of the game.
                foreach (var status in statuses.Where(s => s.Name.Length > 0 && CameraReadiness.IsReceiving(s, now)))
                {
                    watched[status.CameraId] = status.Name;
                }

                next = watched
                    .Select(camera => (Camera: camera, Status: statuses.FirstOrDefault(s => s.CameraId == camera.Key)))
                    .Where(c => !CameraReadiness.IsReceiving(c.Status, now))
                    .Select(c => new CameraVideoLoss(c.Camera.Key, c.Camera.Value, Describe(c.Camera.Value, c.Status, now)))
                    .ToList();
            }

            Apply(next.OrderBy(loss => loss.CameraId).ToList());
        }
    }

    /// <summary>Silences the sound until a camera stops again. The losses are still shown.</summary>
    public void Mute()
    {
        lock (gate)
        {
            muted = true;
            Apply(losses);
        }
    }

    /// <summary>Stops the sound and forgets the cameras, for the next start of the modules.</summary>
    public void Reset()
    {
        lock (gate)
        {
            watched.Clear();
            lastAnswerAt = null;
            losses = [];
            muted = false;
            if (sounding)
            {
                sound.Value?.Silence();
                sounding = false;
            }
        }
    }

    private void Apply(IReadOnlyList<CameraVideoLoss> next)
    {
        var previous = losses.Select(loss => loss.CameraId).ToHashSet();
        foreach (var loss in next.Where(loss => !previous.Contains(loss.CameraId)))
        {
            log.Report($"camera: {loss.Message}");
            // A camera that newly stops is news even after the last alarm was muted.
            muted = false;
        }

        var current = next.Select(loss => loss.CameraId).ToHashSet();
        foreach (var name in watched.Where(camera => previous.Contains(camera.Key) && !current.Contains(camera.Key))
                     .Select(camera => camera.Value))
        {
            log.Report($"camera: {name}の映像が戻りました");
        }

        losses = next;
        if (next.Count == 0)
        {
            muted = false;
        }

        var shouldSound = next.Count > 0 && !muted;
        if (shouldSound && !sounding)
        {
            sound.Value?.Play();
        }
        else if (!shouldSound && sounding)
        {
            sound.Value?.Silence();
        }

        sounding = shouldSound;
    }

    private static string Describe(string name, CameraStatus? status, DateTimeOffset now)
    {
        if (status is null)
        {
            return $"{name}が ball_tracker のカメラの一覧にありません。ball_tracker の --cameras を確かめてください";
        }

        if (!status.Connected)
        {
            return status.DisconnectedAt is { } disconnectedAt
                ? $"{name}が ball_tracker から切断されました ({disconnectedAt.ToDateTimeOffset().ToLocalTime():HH:mm:ss})。USB 接続とアプリを確かめてください"
                : $"{name}が ball_tracker につながっていません。USB 接続とアプリを確かめてください";
        }

        if (status.LastFrameAt is null)
        {
            return $"{name}の映像が ball_tracker に届いていません。スマホの画面が点いているか確かめてください";
        }

        var last = status.LastFrameAt.ToDateTimeOffset();
        var seconds = Math.Max(0, Math.Floor((now - last).TotalSeconds));
        return $"{name}の映像が止まっています (最後に受信: {last.ToLocalTime():HH:mm:ss}、{seconds} 秒前)。スマホの画面が点いているか確かめてください";
    }
}
