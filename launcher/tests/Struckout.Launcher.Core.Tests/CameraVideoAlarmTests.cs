using Google.Protobuf.WellKnownTypes;
using Struckout.Launcher.Core.Launch;
using Struckout.Launcher.Core.Modules;
using Struckout.Launcher.Core.Platform;
using Struckout.Launcher.Core.Spec;
using Tk75Attractions.Struckout.BallTracker.V1;

namespace Struckout.Launcher.Core.Tests;

public sealed class CameraVideoAlarmTests : IDisposable
{
    private static readonly DateTimeOffset Start = new(2026, 11, 1, 5, 0, 0, TimeSpan.Zero);

    private readonly FakeSound sound = new();
    private readonly ListLog log = new();
    private readonly CameraVideoAlarm alarm;

    public CameraVideoAlarmTests() => alarm = new CameraVideoAlarm(() => sound, log);

    public void Dispose() => sound.Dispose();

    [Fact]
    public void DoesNotSoundForCamerasThatHaveNotSentVideoYet()
    {
        // While starting: one phone is open, the other isn't yet
        alarm.Update([Receiving(0, "left", Start), NotConnected(1, "right")], Start);
        alarm.Update([Receiving(0, "left", Start.AddSeconds(10)), NotConnected(1, "right")], Start.AddSeconds(10));

        Assert.Empty(alarm.Losses);
        Assert.Equal(0, sound.Plays);
    }

    [Fact]
    public void SoundsOnceWhileTheVideoStaysStopped()
    {
        alarm.Update([Receiving(0, "left", Start), Receiving(1, "right", Start)], Start);

        // The screen of right went off: its last frame gets older
        for (var seconds = 2; seconds <= 10; seconds += 2)
        {
            var now = Start.AddSeconds(seconds);
            alarm.Update([Receiving(0, "left", now), Receiving(1, "right", Start)], now);
        }

        var loss = Assert.Single(alarm.Losses);
        Assert.Equal("right", loss.Name);
        Assert.Contains("映像が止まっています", loss.Message, StringComparison.Ordinal);
        Assert.Contains("10 秒前", loss.Message, StringComparison.Ordinal);
        Assert.Equal(1, sound.Plays);
        Assert.Equal(0, sound.Silences);
        Assert.Single(log.Lines, line => line.Contains("rightの映像が止まっています", StringComparison.Ordinal));
    }

    [Fact]
    public void WaitsForFramesUpToTheMaxAge()
    {
        alarm.Update([Receiving(0, "left", Start)], Start);

        var now = Start + CameraReadiness.MaxFrameAge;
        alarm.Update([Receiving(0, "left", Start)], now);
        Assert.Empty(alarm.Losses);

        alarm.Update([Receiving(0, "left", Start)], now.AddMilliseconds(1));
        Assert.Single(alarm.Losses);
    }

    [Fact]
    public void SilencesWhenTheVideoComesBack()
    {
        alarm.Update([Receiving(0, "left", Start)], Start);
        alarm.Update([Receiving(0, "left", Start)], Start.AddSeconds(5));

        var now = Start.AddSeconds(7);
        alarm.Update([Receiving(0, "left", now)], now);
        alarm.Update([Receiving(0, "left", now.AddSeconds(1))], now.AddSeconds(1));

        Assert.Empty(alarm.Losses);
        Assert.Equal(1, sound.Plays);
        Assert.Equal(1, sound.Silences);
        Assert.Contains("camera: leftの映像が戻りました", log.Lines);
    }

    [Fact]
    public void KeepsSoundingUntilEveryCameraComesBack()
    {
        alarm.Update([Receiving(0, "left", Start), Receiving(1, "right", Start)], Start);
        var now = Start.AddSeconds(5);
        alarm.Update([Receiving(0, "left", Start), Receiving(1, "right", Start)], now);

        alarm.Update([Receiving(0, "left", now), Receiving(1, "right", Start)], now);

        Assert.Equal(["right"], alarm.Losses.Select(loss => loss.Name));
        Assert.Equal(1, sound.Plays);
        Assert.Equal(0, sound.Silences);
    }

    [Fact]
    public void ReportsADisconnectedCamera()
    {
        alarm.Update([Receiving(0, "left", Start)], Start);

        var status = NotConnected(0, "left");
        status.LastFrameAt = Timestamp.FromDateTimeOffset(Start);
        status.DisconnectedAt = Timestamp.FromDateTimeOffset(Start.AddSeconds(1));
        alarm.Update([status], Start.AddSeconds(2));

        Assert.Contains("ball_tracker から切断されました", Assert.Single(alarm.Losses).Message, StringComparison.Ordinal);
        Assert.Equal(1, sound.Plays);
    }

    [Fact]
    public void ReportsCamerasAfterBallTrackerRestarts()
    {
        alarm.Update([Receiving(0, "left", Start)], Start);

        // A new ball_tracker hasn't received anything yet
        alarm.Update([NotConnected(0, "left")], Start.AddSeconds(2));

        Assert.Single(alarm.Losses);
        Assert.Equal(1, sound.Plays);
    }

    [Fact]
    public void ToleratesOneUnansweredCallButNotLonger()
    {
        alarm.Update([Receiving(0, "left", Start)], Start);

        alarm.Update(null, Start.AddSeconds(2));
        Assert.Empty(alarm.Losses);

        alarm.Update(null, Start.AddSeconds(4));
        Assert.Contains("ball_tracker が動いているか", Assert.Single(alarm.Losses).Message, StringComparison.Ordinal);
        Assert.Equal(1, sound.Plays);
    }

    [Fact]
    public void IgnoresDevicesMissingFromCamerasJson()
    {
        alarm.Update([Receiving(2, "", Start)], Start);
        alarm.Update([NotConnected(2, "")], Start.AddSeconds(10));

        Assert.Empty(alarm.Losses);
        Assert.Equal(0, sound.Plays);
    }

    [Fact]
    public void MuteSilencesUntilAnotherCameraStops()
    {
        alarm.Update([Receiving(0, "left", Start), Receiving(1, "right", Start)], Start);
        var now = Start.AddSeconds(5);
        alarm.Update([Receiving(0, "left", now), Receiving(1, "right", Start)], now);

        alarm.Mute();
        alarm.Update([Receiving(0, "left", now), Receiving(1, "right", Start)], now.AddSeconds(1));
        Assert.True(alarm.IsMuted);
        Assert.Equal((1, 1), (sound.Plays, sound.Silences));

        alarm.Update([Receiving(0, "left", now), Receiving(1, "right", Start)], now.AddSeconds(5));
        Assert.False(alarm.IsMuted);
        Assert.Equal(2, sound.Plays);
    }

    [Fact]
    public void MuteLastsUntilTheVideoComesBack()
    {
        alarm.Update([Receiving(0, "left", Start)], Start);
        alarm.Update([Receiving(0, "left", Start)], Start.AddSeconds(5));
        alarm.Mute();

        var now = Start.AddSeconds(6);
        alarm.Update([Receiving(0, "left", now)], now);
        alarm.Update([Receiving(0, "left", now)], now.AddSeconds(5));

        Assert.Equal(2, sound.Plays);
    }

    [Fact]
    public void ResetSilencesAndForgetsTheCameras()
    {
        alarm.Update([Receiving(0, "left", Start)], Start);
        alarm.Update([Receiving(0, "left", Start)], Start.AddSeconds(5));

        alarm.Reset();
        alarm.Update([NotConnected(0, "left")], Start.AddSeconds(6));

        Assert.Empty(alarm.Losses);
        Assert.Equal((1, 1), (sound.Plays, sound.Silences));
        Assert.DoesNotContain(log.Lines, line => line.Contains("戻りました", StringComparison.Ordinal));
    }

    [Fact]
    public void WorksWithoutSound()
    {
        var silent = new CameraVideoAlarm(() => null, log);

        silent.Update([Receiving(0, "left", Start)], Start);
        silent.Update([Receiving(0, "left", Start)], Start.AddSeconds(5));

        Assert.Single(silent.Losses);
    }

    [Fact]
    public void LooksForAPlayerOnlyWhenItSounds()
    {
        var created = 0;
        var lazy = new CameraVideoAlarm(() => { created++; return sound; }, log);

        lazy.Update([Receiving(0, "left", Start)], Start);
        Assert.Equal(0, created);

        lazy.Update([Receiving(0, "left", Start)], Start.AddSeconds(5));
        lazy.Update([Receiving(0, "left", Start)], Start.AddSeconds(7));
        Assert.Equal(1, created);
    }

    [Fact]
    public async Task StoppingTheCameraModuleSilencesTheAlarm()
    {
        var path = Path.Combine(Path.GetTempPath(), $"camera-alarm-{Guid.NewGuid()}.json");
        await File.WriteAllTextAsync(path, """
            { "package": "camera.highspeed", "cameras": [
              { "serial": "left", "name": "left", "output": "network", "focal_ratio": 1, "pose": [] }
            ] }
            """, TestContext.Current.CancellationToken);
        try
        {
            var runner = new CameraRunner(new Adb("adb", new ConnectedPhone()), path, new ServerPorts(1, 2, 3, 4, 5, 0), alarm);
            await runner.StartAsync(log, TestContext.Current.CancellationToken);
            alarm.Update([Receiving(0, "left", Start)], Start);
            alarm.Update([Receiving(0, "left", Start)], Start.AddSeconds(5));

            await runner.StopAsync(log, TestContext.Current.CancellationToken);

            Assert.Empty(alarm.Losses);
            Assert.Equal((1, 1), (sound.Plays, sound.Silences));
        }
        finally
        {
            File.Delete(path);
        }
    }

    private static CameraStatus Receiving(uint id, string name, DateTimeOffset lastFrameAt) => new()
    {
        CameraId = id,
        Name = name,
        Connected = true,
        Fps = 240,
        LastFrameAt = Timestamp.FromDateTimeOffset(lastFrameAt),
    };

    private static CameraStatus NotConnected(uint id, string name) => new() { CameraId = id, Name = name };

    private sealed class ConnectedPhone : ICommandRunner
    {
        public Task<CommandResult> RunAsync(
            string fileName, IReadOnlyList<string> arguments, CommandOptions? options = null,
            CancellationToken cancellationToken = default) =>
            Task.FromResult(new CommandResult(0, arguments is ["devices"] ? "left\tdevice\n" : ""));
    }

    private sealed class FakeSound : IAlarmSound
    {
        public int Plays { get; private set; }

        public int Silences { get; private set; }

        public void Play() => Plays++;

        public void Silence() => Silences++;

        public void Dispose()
        {
        }
    }
}
