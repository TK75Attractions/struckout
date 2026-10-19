using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Runtime.Versioning;
using Microsoft.Win32.SafeHandles;

namespace Struckout.Launcher.Core.Platform;

/// <summary>
/// A Windows job object that tracks a process and every process it starts.
/// </summary>
/// <remarks>
/// projector starts Sentry's crashpad_handler.exe, which outlives the player for a moment and keeps the files
/// of the module locked. Waiting for the whole job, not only the player, tells when the module is really gone.
/// The job doesn't kill its processes when the launcher exits: a crash of the launcher must not stop a game.
/// https://learn.microsoft.com/windows/win32/procthread/job-objects
/// </remarks>
[SupportedOSPlatform("windows")]
internal sealed partial class ProcessJob : IDisposable
{
    private const int JobObjectBasicAccountingInformation = 1;

    private readonly SafeFileHandle handle;

    private ProcessJob(SafeFileHandle handle) => this.handle = handle;

    public static ProcessJob Create()
    {
        var handle = CreateJobObjectW(IntPtr.Zero, null);
        if (handle.IsInvalid)
        {
            throw new Win32Exception(Marshal.GetLastPInvokeError());
        }

        return new ProcessJob(handle);
    }

    /// <summary>Adds <paramref name="process"/>. The processes it starts from now on join too.</summary>
    public void Add(Process process)
    {
        if (!AssignProcessToJobObject(handle, process.SafeHandle))
        {
            throw new Win32Exception(Marshal.GetLastPInvokeError());
        }
    }

    /// <summary>Waits until every process of the job has exited, at most <paramref name="timeout"/>.</summary>
    /// <returns>Whether they all exited.</returns>
    public async Task<bool> WaitForAllAsync(TimeSpan timeout, CancellationToken cancellationToken)
    {
        var deadline = DateTimeOffset.UtcNow + timeout;
        while (ActiveProcesses() > 0)
        {
            if (DateTimeOffset.UtcNow > deadline)
            {
                return false;
            }

            await Task.Delay(TimeSpan.FromMilliseconds(100), cancellationToken);
        }

        return true;
    }

    public void TerminateAll()
    {
        if (!TerminateJobObject(handle, 1))
        {
            throw new Win32Exception(Marshal.GetLastPInvokeError());
        }
    }

    public void Dispose() => handle.Dispose();

    private uint ActiveProcesses()
    {
        if (!QueryInformationJobObject(
                handle, JobObjectBasicAccountingInformation, out var information,
                (uint)Marshal.SizeOf<BasicAccountingInformation>(), IntPtr.Zero))
        {
            throw new Win32Exception(Marshal.GetLastPInvokeError());
        }

        return information.ActiveProcesses;
    }

    [LibraryImport("kernel32.dll", SetLastError = true, StringMarshalling = StringMarshalling.Utf16)]
    private static partial SafeFileHandle CreateJobObjectW(IntPtr attributes, string? name);

    [LibraryImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static partial bool AssignProcessToJobObject(SafeFileHandle job, SafeProcessHandle process);

    [LibraryImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static partial bool TerminateJobObject(SafeFileHandle job, uint exitCode);

    [LibraryImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static partial bool QueryInformationJobObject(
        SafeFileHandle job, int informationClass, out BasicAccountingInformation information, uint length, IntPtr returnLength);

    // JOBOBJECT_BASIC_ACCOUNTING_INFORMATION
    [StructLayout(LayoutKind.Sequential)]
    private struct BasicAccountingInformation
    {
        public long TotalUserTime;
        public long TotalKernelTime;
        public long ThisPeriodTotalUserTime;
        public long ThisPeriodTotalKernelTime;
        public uint TotalPageFaultCount;
        public uint TotalProcesses;
        public uint ActiveProcesses;
        public uint TotalTerminatedProcesses;
    }
}
