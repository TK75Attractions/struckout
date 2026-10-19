using System.ComponentModel;
using System.Diagnostics;
using System.Text;
using Struckout.Launcher.Core.Modules;
using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.Core.Launch;

/// <summary>ball_tracker, projector and touchpanel: a process on this PC.</summary>
/// <param name="servedPorts">The module is up when these ports are open on this PC.</param>
/// <param name="logFile">
/// Where stdout and stderr go. Null when the module writes its own log (Unity's <c>-logFile</c>).
/// </param>
internal sealed class ProcessModuleRunner(
    ModuleId id,
    HostPlatform platform,
    ICommandRunner runner,
    string executable,
    IReadOnlyList<string> arguments,
    IReadOnlyList<int> servedPorts,
    string? logFile) : IModuleRunner
{
    private static readonly TimeSpan StopTimeout = TimeSpan.FromSeconds(10);

    private Process? process;
    private StreamWriter? log;
    private ProcessJob? job;

    public ModuleId Id { get; } = id;

    public async Task StartAsync(IProgress<string> progress, CancellationToken cancellationToken)
    {
        if (process is { HasExited: false })
        {
            return;
        }

        // It ended by itself
        await ReleaseAsync(cancellationToken);

        if (!File.Exists(executable))
        {
            throw new LauncherException($"{executable} がありません。インストールし直してください");
        }

        // Otherwise the health check would see the other process and report this one as up.
        foreach (var port in servedPorts)
        {
            if (await TcpProbe.IsOpenAsync("127.0.0.1", port, cancellationToken))
            {
                throw new LauncherException(
                    $"{port} 番はほかのプロセスが使っています。前に起動した {Id.Slug()} が残っていないか確かめてください");
            }
        }

        var startInfo = new ProcessStartInfo(executable)
        {
            UseShellExecute = false,
            WorkingDirectory = Path.GetDirectoryName(executable),
            RedirectStandardOutput = logFile is not null,
            RedirectStandardError = logFile is not null,
            StandardOutputEncoding = logFile is null ? null : Encoding.UTF8,
            StandardErrorEncoding = logFile is null ? null : Encoding.UTF8,
            CreateNoWindow = logFile is not null,
        };
        foreach (var argument in arguments)
        {
            startInfo.ArgumentList.Add(argument);
        }

        var started = new Process { StartInfo = startInfo, EnableRaisingEvents = true };
        if (logFile is not null)
        {
            Directory.CreateDirectory(Path.GetDirectoryName(logFile)!);
            var writer = new StreamWriter(logFile, append: true, Encoding.UTF8) { AutoFlush = true };
            log = writer;
            started.OutputDataReceived += (_, e) => Write(writer, e.Data);
            started.ErrorDataReceived += (_, e) => Write(writer, e.Data);
        }

        try
        {
            started.Start();
        }
        catch (Win32Exception e)
        {
            started.Dispose();
            throw new LauncherException($"{executable} を起動できません: {e.Message}", e);
        }

        if (logFile is not null)
        {
            started.BeginOutputReadLine();
            started.BeginErrorReadLine();
        }

        if (OperatingSystem.IsWindows())
        {
            // A child started before this line isn't tracked. Unity starts crashpad_handler.exe only after
            // initializing Sentry, so it's tracked.
            job = ProcessJob.Create();
            job.Add(started);
        }

        process = started;
        progress.Report($"{Id.Slug()}: 起動しました (PID {started.Id}{(logFile is null ? "" : $"、ログ: {logFile}")})");
    }

    public async Task<Health> CheckAsync(CancellationToken cancellationToken)
    {
        if (process is null)
        {
            return Health.Down("起動していません");
        }

        if (process.HasExited)
        {
            return Health.Exited($"終了しました (終了コード {process.ExitCode})");
        }

        return servedPorts.Count == 0
            ? Health.Up()
            : await TcpProbe.CheckAsync("127.0.0.1", servedPorts, cancellationToken);
    }

    /// <summary>Asks the process to quit, and kills it if it doesn't within <see cref="StopTimeout"/>.</summary>
    public async Task StopAsync(IProgress<string> progress, CancellationToken cancellationToken)
    {
        if (process is null)
        {
            return;
        }

        if (!process.HasExited)
        {
            progress.Report($"{Id.Slug()}: 止めます");
            await RequestExitAsync(process, cancellationToken);
            using var timeout = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
            timeout.CancelAfter(StopTimeout);
            try
            {
                await process.WaitForExitAsync(timeout.Token);
            }
            catch (OperationCanceledException) when (!cancellationToken.IsCancellationRequested)
            {
                progress.Report($"{Id.Slug()}: 止まらないので強制終了します");
                process.Kill(entireProcessTree: true);
            }
        }

        if (OperatingSystem.IsWindows() && job is not null && !await job.WaitForAllAsync(StopTimeout, cancellationToken))
        {
            progress.Report($"{Id.Slug()}: 子プロセスが残っているので強制終了します");
            job.TerminateAll();
        }

        await ReleaseAsync(cancellationToken);
    }

    private async Task ReleaseAsync(CancellationToken cancellationToken)
    {
        if (process is not null)
        {
            // Also waits until the output handlers have written everything, so the log can be closed.
            await process.WaitForExitAsync(cancellationToken);
            process.Dispose();
            process = null;
        }

        if (OperatingSystem.IsWindows())
        {
            job?.Dispose();
        }

        job = null;
        if (log is not null)
        {
            await log.DisposeAsync();
            log = null;
        }
    }

    /// <summary>SIGTERM on Unix; closing the main window on Windows (Unity and Slint quit on it).</summary>
    private async Task RequestExitAsync(Process target, CancellationToken cancellationToken)
    {
        if (platform.IsWindows)
        {
            // Console programs have no window. They are killed after the timeout.
            target.CloseMainWindow();
            return;
        }

        await runner.RunAsync("kill", ["-TERM", target.Id.ToString(System.Globalization.CultureInfo.InvariantCulture)],
            cancellationToken: cancellationToken);
    }

    private static void Write(StreamWriter writer, string? line)
    {
        if (line is null)
        {
            return;
        }

        lock (writer)
        {
            writer.WriteLine(line);
        }
    }
}
