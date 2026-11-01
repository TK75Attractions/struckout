using System.Runtime.InteropServices;
using System.Runtime.Versioning;

namespace Struckout.Launcher.Core.Platform;

/// <summary>Repeats a warning sound until it's stopped.</summary>
/// <remarks>
/// It's an interface because each OS plays sound differently (see <see cref="AlarmSound.Create"/>),
/// and so that the alarm can be tested without speakers.
/// </remarks>
public interface IAlarmSound : IDisposable
{
    /// <summary>Starts repeating the sound. Does nothing while it's already playing.</summary>
    void Play();

    /// <summary>Stops the sound. Does nothing while it isn't playing.</summary>
    void Silence();
}

public static class AlarmSound
{
    /// <summary>Writes <see cref="AlarmTone"/> to <paramref name="file"/> and finds a way to play it on this OS.</summary>
    /// <returns>null when this PC can't play sound, which has been reported to <paramref name="log"/>.</returns>
    public static IAlarmSound? Create(HostPlatform platform, string file, ICommandRunner runner, IProgress<string> log)
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(file)!);
            // Written every time, so that it follows AlarmTone after an update
            File.WriteAllBytes(file, AlarmTone.Wav());
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            log.Report($"警告音のファイル {file} を書けないので、警告音は鳴りません: {e.Message}");
            return null;
        }

        if (platform.Os == OperatingSystemKind.Windows)
        {
            // guaranteed by HostPlatform.Detect
            return OperatingSystem.IsWindows()
                ? new WindowsAlarmSound(file, log)
                : throw new InvalidOperationException("HostPlatform should be Windows only on Windows");
        }

        var players = PlayerNames(platform.Os);
        var player = players.Select(ExecutableSearch.OnPath).FirstOrDefault(path => path is not null);
        if (player is null)
        {
            log.Report($"{string.Join("、", players)} のどれも見つからないので、警告音は鳴りません");
            return null;
        }

        return new CommandAlarmSound(player, file, runner, log);
    }

    /// <summary>The commands that play a WAV file on <paramref name="os"/>, preferred first.</summary>
    /// <remarks>
    /// Linux: PulseAudio's paplay also works on PipeWire with pipewire-pulse, pw-play on PipeWire without it,
    /// and ALSA's aplay where there's no sound server. macOS has afplay. Windows plays with an API instead.
    /// </remarks>
    internal static IReadOnlyList<string> PlayerNames(OperatingSystemKind os) => os switch
    {
        OperatingSystemKind.Linux => ["paplay", "pw-play", "aplay"],
        OperatingSystemKind.MacOS => ["afplay"],
        OperatingSystemKind.Windows => [],
        _ => throw new InvalidOperationException($"unknown OS {os}"),
    };
}

/// <summary>Plays the file with a command again and again, which none of the players can loop by itself.</summary>
internal sealed class CommandAlarmSound(string player, string file, ICommandRunner runner, IProgress<string> log) : IAlarmSound
{
    private readonly Lock gate = new();
    private CancellationTokenSource? playing;
    private bool failureReported;

    /// <summary>The loop of the latest <see cref="Play"/>, for tests.</summary>
    internal Task Loop { get; private set; } = Task.CompletedTask;

    public void Play()
    {
        lock (gate)
        {
            if (playing is not null)
            {
                return;
            }

            playing = new CancellationTokenSource();
            Loop = LoopAsync(playing);
        }
    }

    public void Silence()
    {
        CancellationTokenSource? source;
        lock (gate)
        {
            source = playing;
            playing = null;
        }

        // Cancelling kills the player (ProcessCommandRunner), so the sound stops at once.
        source?.Cancel();
        source?.Dispose();
    }

    public void Dispose() => Silence();

    private async Task LoopAsync(CancellationTokenSource source)
    {
        var cancellationToken = source.Token;
        try
        {
            while (true)
            {
                string? failure;
                try
                {
                    var result = await runner.RunAsync(player, [file], cancellationToken: cancellationToken);
                    failure = result.Succeeded ? null : result.Output.Trim();
                }
                catch (CommandNotFoundException e)
                {
                    failure = e.Message;
                }

                if (failure is not null)
                {
                    if (!failureReported)
                    {
                        log.Report($"警告音を鳴らせません ({Path.GetFileName(player)}): {failure}");
                        failureReported = true;
                    }

                    // A player that fails at once would otherwise be restarted without a pause.
                    await Task.Delay(AlarmTone.Duration, cancellationToken);
                }
            }
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            // Stopped
        }
        finally
        {
            // Only when the loop ended by an unexpected exception, so that Play can start it again
            lock (gate)
            {
                if (playing == source)
                {
                    playing = null;
                }
            }
        }
    }
}

/// <summary>Loops the file with PlaySound, the API Windows has for it, since Windows has no command to play sound.</summary>
/// <remarks>https://learn.microsoft.com/previous-versions/dd743680(v=vs.85)</remarks>
[SupportedOSPlatform("windows")]
internal sealed partial class WindowsAlarmSound(string file, IProgress<string> log) : IAlarmSound
{
    private const uint SndAsync = 0x0001;
    private const uint SndNoDefault = 0x0002;
    private const uint SndLoop = 0x0008;
    private const uint SndFilename = 0x00020000;

    private readonly Lock gate = new();
    private bool playing;

    public void Play()
    {
        lock (gate)
        {
            if (playing)
            {
                return;
            }

            // SND_NODEFAULT: fail rather than play the default beep, which doesn't sound like an alarm
            if (!PlaySound(file, IntPtr.Zero, SndAsync | SndNoDefault | SndLoop | SndFilename))
            {
                log.Report($"警告音を鳴らせません ({file} を再生できません)");
                return;
            }

            playing = true;
        }
    }

    public void Silence()
    {
        lock (gate)
        {
            if (!playing)
            {
                return;
            }

            // A null sound stops the sound playing asynchronously.
            PlaySound(null, IntPtr.Zero, 0);
            playing = false;
        }
    }

    public void Dispose() => Silence();

    [LibraryImport("winmm.dll", EntryPoint = "PlaySoundW", StringMarshalling = StringMarshalling.Utf16)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static partial bool PlaySound(string? sound, IntPtr module, uint flags);
}
