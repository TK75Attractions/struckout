using System.Buffers.Binary;

namespace Struckout.Launcher.Core.Platform;

/// <summary>The warning sound as a WAV file, generated so that no binary asset has to be committed.</summary>
/// <remarks>
/// Two short beeps and a pause, so that playing it in a loop sounds like an alarm.
/// It's 16-bit PCM, the one format every player supports (paplay, pw-play, aplay, afplay and PlaySound).
/// https://learn.microsoft.com/windows/win32/multimedia/waveform-audio-file-format
/// </remarks>
internal static class AlarmTone
{
    internal const int SampleRate = 16000;

    private const double Frequency = 880;
    private const double Amplitude = 0.5;

    private static readonly TimeSpan Beep = TimeSpan.FromMilliseconds(150);
    private static readonly TimeSpan Gap = TimeSpan.FromMilliseconds(80);

    /// <summary>Fades the beeps in and out, without which they click.</summary>
    private static readonly TimeSpan Fade = TimeSpan.FromMilliseconds(5);

    /// <summary>The length of one play, including the pause before the next one.</summary>
    internal static readonly TimeSpan Duration = TimeSpan.FromSeconds(1);

    internal static byte[] Wav()
    {
        var samples = Samples();
        var dataSize = samples.Length * sizeof(short);
        var wav = new byte[44 + dataSize];
        var span = wav.AsSpan();
        "RIFF"u8.CopyTo(span);
        BinaryPrimitives.WriteInt32LittleEndian(span[4..], 36 + dataSize);
        "WAVE"u8.CopyTo(span[8..]);
        "fmt "u8.CopyTo(span[12..]);
        BinaryPrimitives.WriteInt32LittleEndian(span[16..], 16);
        // PCM, mono
        BinaryPrimitives.WriteInt16LittleEndian(span[20..], 1);
        BinaryPrimitives.WriteInt16LittleEndian(span[22..], 1);
        BinaryPrimitives.WriteInt32LittleEndian(span[24..], SampleRate);
        BinaryPrimitives.WriteInt32LittleEndian(span[28..], SampleRate * sizeof(short));
        BinaryPrimitives.WriteInt16LittleEndian(span[32..], sizeof(short));
        BinaryPrimitives.WriteInt16LittleEndian(span[34..], 16);
        "data"u8.CopyTo(span[36..]);
        BinaryPrimitives.WriteInt32LittleEndian(span[40..], dataSize);
        for (var i = 0; i < samples.Length; i++)
        {
            BinaryPrimitives.WriteInt16LittleEndian(span[(44 + (i * sizeof(short)))..], samples[i]);
        }

        return wav;
    }

    private static short[] Samples()
    {
        var samples = new short[ToSamples(Duration)];
        var beep = ToSamples(Beep);
        var fade = ToSamples(Fade);
        foreach (var start in new[] { 0, beep + ToSamples(Gap) })
        {
            for (var i = 0; i < beep; i++)
            {
                var envelope = Math.Min(1.0, Math.Min(i, beep - 1 - i) / (double)fade);
                var value = Amplitude * envelope * Math.Sin(2 * Math.PI * Frequency * i / SampleRate);
                samples[start + i] = (short)Math.Round(value * short.MaxValue);
            }
        }

        return samples;
    }

    private static int ToSamples(TimeSpan time) => (int)(time.TotalSeconds * SampleRate);
}
