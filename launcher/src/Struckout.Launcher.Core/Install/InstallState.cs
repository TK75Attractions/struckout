using System.Text.Json;
using System.Text.Json.Serialization;
using Struckout.Launcher.Core.Modules;

namespace Struckout.Launcher.Core.Install;

/// <summary>A tool that some modules need but that we don't build.</summary>
public enum PrerequisiteId
{
    /// <summary>Runs game_master and its database.</summary>
    Docker,

    /// <summary>Android Platform Tools: installs and controls the camera app.</summary>
    Adb,

    /// <summary>ball_tracker decodes the videos the phones send.</summary>
    Ffmpeg,
}

public enum PackageManager
{
    Winget,
    Homebrew,
    HomebrewCask,
}

/// <summary>How a prerequisite got onto this PC. Uninstalling removes only what we installed.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "kind")]
[JsonDerivedType(typeof(FoundPrerequisite), "found")]
[JsonDerivedType(typeof(PackageManagerPrerequisite), "package-manager")]
[JsonDerivedType(typeof(DownloadedPrerequisite), "downloaded")]
public abstract record PrerequisiteOrigin;

/// <summary>It was already installed. We leave it alone.</summary>
public sealed record FoundPrerequisite(string Path) : PrerequisiteOrigin;

/// <summary>We installed it with <paramref name="Manager"/>.</summary>
public sealed record PackageManagerPrerequisite(PackageManager Manager, string PackageId) : PrerequisiteOrigin;

/// <summary>We extracted it into <paramref name="Directory"/>.</summary>
public sealed record DownloadedPrerequisite(string Directory) : PrerequisiteOrigin;

/// <summary>What was installed for a module.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "kind")]
[JsonDerivedType(typeof(ExecutableInstallation), "executable")]
[JsonDerivedType(typeof(ComposeInstallation), "compose")]
[JsonDerivedType(typeof(ApkInstallation), "apk")]
public abstract record ModuleInstallation
{
    /// <summary>The release tag and when it was published, to tell whether an update is available.</summary>
    public required string ReleaseTag { get; init; }

    public required DateTimeOffset ReleasePublishedAt { get; init; }

    /// <summary>Everything we put on this PC for the module.</summary>
    public required string Directory { get; init; }
}

/// <summary>ball_tracker, touchpanel and projector: a program we start.</summary>
public sealed record ExecutableInstallation : ModuleInstallation
{
    public required string Executable { get; init; }
}

/// <summary>game_master: compose files in <see cref="ModuleInstallation.Directory"/> and a pulled image.</summary>
public sealed record ComposeInstallation : ModuleInstallation;

/// <summary>The camera app, installed on phones over adb.</summary>
public sealed record ApkInstallation : ModuleInstallation
{
    public required string Package { get; init; }

    /// <summary>The serials of the phones the app was installed on.</summary>
    public required IReadOnlyList<string> Devices { get; init; }

    /// <summary>Phones using a differently signed existing app, rather than the recorded release. Optional in older state files.</summary>
    public IReadOnlyList<string> ExistingDevices { get; init; } = [];
}

/// <summary>What the installer has put on this PC, saved as <c>install-state.json</c>.</summary>
public sealed class InstallState
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true,
        PropertyNamingPolicy = JsonNamingPolicy.KebabCaseLower,
        DictionaryKeyPolicy = JsonNamingPolicy.KebabCaseLower,
        Converters = { new JsonStringEnumConverter(JsonNamingPolicy.KebabCaseLower) },
    };

    public Dictionary<ModuleId, ModuleInstallation> Modules { get; init; } = [];

    public Dictionary<PrerequisiteId, PrerequisiteOrigin> Prerequisites { get; init; } = [];

    /// <summary>An empty state if the file doesn't exist.</summary>
    /// <exception cref="LauncherException">The file is broken.</exception>
    public static InstallState Load(string path)
    {
        if (!File.Exists(path))
        {
            return new InstallState();
        }

        try
        {
            return Parse(File.ReadAllText(path));
        }
        catch (JsonException e)
        {
            throw new LauncherException($"{path} が壊れています: {e.Message}", e);
        }
    }

    public void Save(string path)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(Path.GetFullPath(path))!);
        var partial = path + ".tmp";
        File.WriteAllText(partial, Serialize());
        File.Move(partial, path, overwrite: true);
    }

    internal static InstallState Parse(string json) =>
        JsonSerializer.Deserialize<InstallState>(json, JsonOptions) ?? new InstallState();

    internal string Serialize() => JsonSerializer.Serialize(this, JsonOptions);
}
