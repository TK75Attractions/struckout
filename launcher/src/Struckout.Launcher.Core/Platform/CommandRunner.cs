using System.ComponentModel;
using System.Diagnostics;
using System.Text;

namespace Struckout.Launcher.Core.Platform;

/// <summary>The exit code and the merged stdout/stderr of a finished command.</summary>
public sealed record CommandResult(int ExitCode, string Output)
{
    public bool Succeeded => ExitCode == 0;
}

public sealed record CommandOptions
{
    public string? WorkingDirectory { get; init; }

    public IReadOnlyDictionary<string, string> Environment { get; init; } = new Dictionary<string, string>();
}

/// <summary>
/// Runs short-lived external commands (adb, docker, winget, ...) and collects their output.
/// </summary>
/// <remarks>
/// It's an interface so that the install and launch steps can be tested without the real tools.
/// </remarks>
public interface ICommandRunner
{
    /// <exception cref="CommandNotFoundException">The executable can't be started.</exception>
    Task<CommandResult> RunAsync(
        string fileName, IReadOnlyList<string> arguments, CommandOptions? options = null,
        CancellationToken cancellationToken = default);
}

/// <summary>The executable of a command doesn't exist or can't be executed.</summary>
public sealed class CommandNotFoundException(string fileName, Exception inner)
    : Exception($"{fileName} を実行できません: {inner.Message}", inner)
{
    public string FileName { get; } = fileName;
}

public sealed class ProcessCommandRunner : ICommandRunner
{
    public async Task<CommandResult> RunAsync(
        string fileName, IReadOnlyList<string> arguments, CommandOptions? options = null,
        CancellationToken cancellationToken = default)
    {
        options ??= new CommandOptions();
        var startInfo = new ProcessStartInfo(fileName)
        {
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
            CreateNoWindow = true,
            StandardOutputEncoding = Encoding.UTF8,
            StandardErrorEncoding = Encoding.UTF8,
        };
        foreach (var argument in arguments)
        {
            startInfo.ArgumentList.Add(argument);
        }

        if (options.WorkingDirectory is not null)
        {
            startInfo.WorkingDirectory = options.WorkingDirectory;
        }

        foreach (var (key, value) in options.Environment)
        {
            startInfo.Environment[key] = value;
        }

        using var process = new Process { StartInfo = startInfo };
        var output = new StringBuilder();
        process.OutputDataReceived += (_, e) => Append(e.Data);
        process.ErrorDataReceived += (_, e) => Append(e.Data);
        try
        {
            process.Start();
        }
        catch (Win32Exception e)
        {
            throw new CommandNotFoundException(fileName, e);
        }

        process.BeginOutputReadLine();
        process.BeginErrorReadLine();
        try
        {
            await process.WaitForExitAsync(cancellationToken);
        }
        catch (OperationCanceledException)
        {
            process.Kill(entireProcessTree: true);
            throw;
        }

        lock (output)
        {
            return new CommandResult(process.ExitCode, output.ToString());
        }

        void Append(string? line)
        {
            if (line is null)
            {
                return;
            }

            lock (output)
            {
                output.AppendLine(line);
            }
        }
    }
}
