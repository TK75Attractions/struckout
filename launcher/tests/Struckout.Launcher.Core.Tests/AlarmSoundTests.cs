using System.Buffers.Binary;
using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.Core.Tests;

public sealed class AlarmSoundTests
{
    private static readonly TimeSpan Timeout = TimeSpan.FromSeconds(10);

    [Fact]
    public void ToneIsPcmWav()
    {
        var wav = AlarmTone.Wav().AsSpan();

        Assert.True(wav[..4].SequenceEqual("RIFF"u8));
        Assert.True(wav[8..16].SequenceEqual("WAVEfmt "u8));
        Assert.Equal(1, BinaryPrimitives.ReadInt16LittleEndian(wav[20..]));
        Assert.Equal(AlarmTone.SampleRate, BinaryPrimitives.ReadInt32LittleEndian(wav[24..]));
        Assert.True(wav[36..40].SequenceEqual("data"u8));
        var dataSize = BinaryPrimitives.ReadInt32LittleEndian(wav[40..]);
        Assert.Equal(wav.Length - 44, dataSize);
        Assert.Equal(wav.Length - 8, BinaryPrimitives.ReadInt32LittleEndian(wav[4..]));
        Assert.Equal(AlarmTone.Duration.TotalSeconds * AlarmTone.SampleRate, dataSize / sizeof(short));
        Assert.Contains(wav[44..].ToArray(), sample => sample != 0);
    }

    [Theory]
    [InlineData(OperatingSystemKind.Linux, new[] { "paplay", "pw-play", "aplay" })]
    [InlineData(OperatingSystemKind.MacOS, new[] { "afplay" })]
    [InlineData(OperatingSystemKind.Windows, new string[0])]
    public void PlaysWithTheCommandsOfTheOs(OperatingSystemKind os, string[] players) =>
        Assert.Equal(players, AlarmSound.PlayerNames(os));

    [Fact]
    public async Task RepeatsThePlayerUntilSilenced()
    {
        var runner = new BlockingRunner();
        using var sound = new CommandAlarmSound("/usr/bin/paplay", "alarm.wav", runner, new ListLog());

        sound.Play();
        await runner.Started.WaitAsync(Timeout, TestContext.Current.CancellationToken);
        // Already playing: no second loop
        sound.Play();
        runner.Finish();
        await runner.Started.WaitAsync(Timeout, TestContext.Current.CancellationToken);

        sound.Silence();
        await sound.Loop.WaitAsync(Timeout, TestContext.Current.CancellationToken);

        Assert.Equal(2, runner.Calls.Count);
        Assert.All(runner.Calls, call => Assert.Equal(("/usr/bin/paplay", "alarm.wav"), call));
        Assert.Equal(1, runner.Cancelled);
    }

    [Fact]
    public async Task PlaysAgainAfterSilenced()
    {
        var runner = new BlockingRunner();
        using var sound = new CommandAlarmSound("aplay", "alarm.wav", runner, new ListLog());

        sound.Play();
        await runner.Started.WaitAsync(Timeout, TestContext.Current.CancellationToken);
        sound.Silence();
        await sound.Loop.WaitAsync(Timeout, TestContext.Current.CancellationToken);
        sound.Play();
        await runner.Started.WaitAsync(Timeout, TestContext.Current.CancellationToken);
        sound.Silence();
        await sound.Loop.WaitAsync(Timeout, TestContext.Current.CancellationToken);

        Assert.Equal(2, runner.Calls.Count);
    }

    [Fact]
    public async Task ReportsAFailingPlayerOnce()
    {
        var runner = new FailingRunner();
        var log = new ListLog();
        using var sound = new CommandAlarmSound("paplay", "alarm.wav", runner, log);

        sound.Play();
        await runner.Failed.WaitAsync(Timeout, TestContext.Current.CancellationToken);
        await runner.Failed.WaitAsync(Timeout, TestContext.Current.CancellationToken);
        sound.Silence();
        await sound.Loop.WaitAsync(Timeout, TestContext.Current.CancellationToken);

        Assert.Equal("警告音を鳴らせません (paplay): Connection refused", Assert.Single(log.Lines));
    }

    [Fact]
    public void SilenceWithoutPlayDoesNothing()
    {
        using var sound = new CommandAlarmSound("paplay", "alarm.wav", new BlockingRunner(), new ListLog());

        sound.Silence();
        sound.Silence();
    }

    /// <summary>A player that plays until the test finishes it or the sound is silenced.</summary>
    private sealed class BlockingRunner : ICommandRunner
    {
        private readonly List<(string, string)> calls = [];
        private TaskCompletionSource finished = new(TaskCreationOptions.RunContinuationsAsynchronously);

        public SemaphoreSlim Started { get; } = new(0);

        public int Cancelled { get; private set; }

        public List<(string, string)> Calls
        {
            get
            {
                lock (calls)
                {
                    return calls.ToList();
                }
            }
        }

        public void Finish() => finished.SetResult();

        public async Task<CommandResult> RunAsync(
            string fileName, IReadOnlyList<string> arguments, CommandOptions? options = null,
            CancellationToken cancellationToken = default)
        {
            lock (calls)
            {
                calls.Add((fileName, Assert.Single(arguments)));
            }

            var current = finished;
            Started.Release();
            try
            {
                await current.Task.WaitAsync(cancellationToken);
            }
            catch (OperationCanceledException)
            {
                Cancelled++;
                throw;
            }

            finished = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
            return new CommandResult(0, "");
        }
    }

    private sealed class FailingRunner : ICommandRunner
    {
        public SemaphoreSlim Failed { get; } = new(0);

        public Task<CommandResult> RunAsync(
            string fileName, IReadOnlyList<string> arguments, CommandOptions? options = null,
            CancellationToken cancellationToken = default)
        {
            Failed.Release();
            return Task.FromResult(new CommandResult(1, "Connection refused\n"));
        }
    }
}
