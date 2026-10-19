using System.Runtime.InteropServices;

namespace Struckout.Launcher.Core.Platform;

public enum OperatingSystemKind
{
    Windows,
    MacOS,
    Linux,
}

public enum CpuArchitecture
{
    X64,
    Arm64,
}

/// <summary>The OS and CPU the launcher runs on.</summary>
public sealed record HostPlatform(OperatingSystemKind Os, CpuArchitecture Architecture)
{
    public static HostPlatform Current { get; } = Detect();

    public bool IsWindows => Os == OperatingSystemKind.Windows;

    /// <summary>
    /// Suffixes of the release assets (<c>&lt;module&gt;-&lt;suffix&gt;</c>) this host can run, preferred first.
    /// </summary>
    /// <remarks>
    /// The names follow the "Rename binaries" step of <c>.github/workflows/nightly_release.yaml</c>.
    /// Windows on Arm runs x86_64 binaries through emulation, so it falls back to them until
    /// native arm64 builds are published. macOS on Intel and Linux on Arm have no fallback.
    /// </remarks>
    public IReadOnlyList<string> ReleaseAssetSuffixes => (Os, Architecture) switch
    {
        (OperatingSystemKind.Windows, CpuArchitecture.X64) => ["x86_64-windows"],
        (OperatingSystemKind.Windows, CpuArchitecture.Arm64) => ["arm64-windows", "x86_64-windows"],
        (OperatingSystemKind.MacOS, CpuArchitecture.X64) => ["x86_64-macos"],
        (OperatingSystemKind.MacOS, CpuArchitecture.Arm64) => ["arm64-macos"],
        (OperatingSystemKind.Linux, CpuArchitecture.X64) => ["x86_64-linux"],
        (OperatingSystemKind.Linux, CpuArchitecture.Arm64) => ["arm64-linux"],
        _ => throw new InvalidOperationException($"unknown platform {this}"),
    };

    /// <summary>The file name of an executable called <paramref name="name"/> on this OS.</summary>
    public string ExecutableName(string name) => IsWindows ? name + ".exe" : name;

    private static HostPlatform Detect()
    {
        OperatingSystemKind os;
        if (OperatingSystem.IsWindows())
        {
            os = OperatingSystemKind.Windows;
        }
        else if (OperatingSystem.IsMacOS())
        {
            os = OperatingSystemKind.MacOS;
        }
        else if (OperatingSystem.IsLinux())
        {
            os = OperatingSystemKind.Linux;
        }
        else
        {
            throw new PlatformNotSupportedException("Windows、macOS、Linux 以外では動きません");
        }

        var architecture = RuntimeInformation.OSArchitecture switch
        {
            System.Runtime.InteropServices.Architecture.X64 => CpuArchitecture.X64,
            System.Runtime.InteropServices.Architecture.Arm64 => CpuArchitecture.Arm64,
            var other => throw new PlatformNotSupportedException($"{other} の CPU では動きません"),
        };
        return new HostPlatform(os, architecture);
    }
}
