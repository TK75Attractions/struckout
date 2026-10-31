using System.Text.RegularExpressions;
using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.Core.Modules;

internal enum ScreenState { Unknown, Awake, Asleep, Locked }

/// <summary>Talks to the phones over adb.</summary>
public sealed class Adb(string executable, ICommandRunner runner)
{
    internal async Task<ScreenState> ScreenStateAsync(string serial, CancellationToken cancellationToken)
    {
        var power = await RunAsync(serial, ["shell", "dumpsys", "power"], cancellationToken);
        var window = await RunAsync(serial, ["shell", "dumpsys", "window"], cancellationToken);
        return ParseScreenState(power.Succeeded ? power.Output : "", window.Succeeded ? window.Output : "");
    }

    /// <summary>Recognizes explicit Android state fields; missing or unfamiliar fields remain unknown.</summary>
    internal static ScreenState ParseScreenState(string power, string window)
    {
        var wakefulness = Regex.Match(power, @"\bmWakefulness=(Awake|Asleep|Dozing|Dreaming)\b");
        // Generic 'showing' is only meaningful inside KeyguardServiceDelegate's state block.
        var explicitKeyguard = Regex.Matches(window,
            @"\b(?:mShowingLockscreen|mKeyguardShowing|isStatusBarKeyguard|mDreamingLockscreen)=(true|false)\b");
        // AOSP KeyguardServiceDelegate.dump prints the heading without a colon.
        // https://github.com/aosp-mirror/platform_frameworks_base/blob/main/services/core/java/com/android/server/policy/keyguard/KeyguardServiceDelegate.java
        var delegateState = Regex.Match(window, @"(?m)^[ \t]*KeyguardServiceDelegate:?[ \t]*\r?\n[ \t]+showing=(true|false)\b");
        if (explicitKeyguard.Any(m => m.Groups[1].Value == "true") || delegateState.Groups[1].Value == "true")
            return ScreenState.Locked;
        if (wakefulness.Success && wakefulness.Groups[1].Value != "Awake")
            return ScreenState.Asleep;
        if (wakefulness.Success && (explicitKeyguard.Count > 0 || delegateState.Success))
            return ScreenState.Awake;
        return ScreenState.Unknown;
    }

    /// <summary>The serials of the phones that are connected and authorized.</summary>
    public async Task<IReadOnlyList<string>> DevicesAsync(CancellationToken cancellationToken)
    {
        var result = await runner.RunAsync(executable, ["devices"], cancellationToken: cancellationToken);
        if (!result.Succeeded)
        {
            throw new LauncherException($"adb devices が失敗しました: {result.Output.Trim()}");
        }

        return ParseDevices(result.Output);
    }

    /// <summary>The models of the connected and authorized phones by serial, to tell the user which phone is which.</summary>
    public async Task<IReadOnlyDictionary<string, string>> ModelsAsync(CancellationToken cancellationToken)
    {
        var result = await runner.RunAsync(executable, ["devices", "-l"], cancellationToken: cancellationToken);
        if (!result.Succeeded)
        {
            throw new LauncherException($"adb devices -l が失敗しました: {result.Output.Trim()}");
        }

        return ParseModels(result.Output);
    }

    public Task<CommandResult> RunAsync(string serial, IReadOnlyList<string> arguments, CancellationToken cancellationToken) =>
        runner.RunAsync(executable, ["-s", serial, .. arguments], cancellationToken: cancellationToken);

    public async Task<string> ShellAsync(string serial, IReadOnlyList<string> arguments, CancellationToken cancellationToken) =>
        (await RunAsync(serial, ["shell", .. arguments], cancellationToken)).Output;

    /// <summary>Reads the output of <c>adb devices</c>. Unauthorized and offline phones are left out.</summary>
    internal static IReadOnlyList<string> ParseDevices(string output) =>
        output.Split('\n')
            .Select(line => line.TrimEnd('\r').Split('\t'))
            .Where(columns => columns.Length == 2 && columns[1] == "device")
            .Select(columns => columns[0])
            .ToList();

    /// <summary>
    /// Reads the output of <c>adb devices -l</c>, whose columns are separated by spaces and whose model has
    /// underscores for spaces (<c>model:Pixel_7</c>).
    /// </summary>
    internal static IReadOnlyDictionary<string, string> ParseModels(string output) =>
        output.Split('\n')
            .Select(line => line.Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries))
            .Where(columns => columns.Length >= 2 && columns[1] == "device")
            .Select(columns => (Serial: columns[0], Model: columns.FirstOrDefault(c => c.StartsWith("model:", StringComparison.Ordinal))))
            .Where(device => device.Model is not null)
            .ToDictionary(device => device.Serial, device => device.Model!["model:".Length..].Replace('_', ' '), StringComparer.Ordinal);
}
