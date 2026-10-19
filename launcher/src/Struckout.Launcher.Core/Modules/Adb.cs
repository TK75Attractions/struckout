using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.Core.Modules;

/// <summary>Talks to the phones over adb.</summary>
public sealed class Adb(string executable, ICommandRunner runner)
{
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
}
