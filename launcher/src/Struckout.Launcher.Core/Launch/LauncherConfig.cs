using System.Text.Json;
using Struckout.Launcher.Core.Modules;
using Tomlyn;
using Tomlyn.Serialization;

namespace Struckout.Launcher.Core.Launch;

public enum CollisionOutput
{
    /// <summary>Send collisions to projector.</summary>
    Network,

    /// <summary>Write collisions to a CSV file, to test without projector.</summary>
    Csv,
}

/// <param name="CsvPath">Where to write when <paramref name="Output"/> is <see cref="CollisionOutput.Csv"/>.</param>
/// <param name="ExtraArguments">
/// Passed to ball_tracker as is. ball_tracker has many tuning options and they change often, so the launcher
/// doesn't mirror them.
/// </param>
public sealed record BallTrackerSettings(CollisionOutput Output, string? CsvPath, IReadOnlyList<string> ExtraArguments);

/// <param name="Monitor">The display to show the target on, from 1.</param>
/// <param name="CollisionTransform">The file saved on the F2 screen of projector. Null for projector's default.</param>
public sealed record ProjectorSettings(int Monitor, bool Fullscreen, string? CollisionTransform);

/// <summary>What to start on this PC and how, read from <c>launcher.toml</c>.</summary>
/// <remarks>
/// The launcher takes no options on the command line: a PC at a booth is set up once and started many times,
/// so the settings live in a file the user can read and edit.
/// </remarks>
/// <param name="Modules">The modules to start on this PC.</param>
/// <param name="RemoteHosts">Where the modules running on other PCs are.</param>
public sealed record LauncherConfig(
    uint MachineId,
    IReadOnlySet<ModuleId> Modules,
    IReadOnlyDictionary<ModuleId, string> RemoteHosts,
    BallTrackerSettings BallTracker,
    ProjectorSettings Projector)
{
    /// <summary>The host to connect to <paramref name="target"/> at.</summary>
    public string HostOf(ModuleId target) =>
        Modules.Contains(target)
            ? "127.0.0.1"
            : RemoteHosts.TryGetValue(target, out var host)
                ? host
                : throw new ArgumentOutOfRangeException(nameof(target), target, "only servers have a host");

    /// <exception cref="LauncherException">The file is missing or wrong.</exception>
    public static LauncherConfig Load(string path)
    {
        string text;
        try
        {
            text = File.ReadAllText(path);
        }
        catch (Exception e) when (e is FileNotFoundException or DirectoryNotFoundException)
        {
            throw new LauncherException($"{path} がありません。先にモジュールをインストールしてください", e);
        }

        return Parse(text, path);
    }

    /// <param name="path">Relative paths in the settings are relative to this file.</param>
    /// <exception cref="LauncherException">The text is not valid TOML or has wrong values.</exception>
    internal static LauncherConfig Parse(string text, string path)
    {
        Document document;
        try
        {
            document = TomlSerializer.Deserialize<Document>(text, new TomlSerializerOptions
            {
                PropertyNamingPolicy = JsonNamingPolicy.KebabCaseLower,
                SourceName = path,
            }) ?? new Document();
        }
        catch (TomlException e)
        {
            throw new LauncherException($"{path} を読めません: {e.Message}", e);
        }

        var errors = new List<string>();
        Unknown(document.Unknown, "");
        Unknown(document.Hosts.Unknown, "hosts.");
        Unknown(document.BallTracker.Unknown, "ball-tracker.");
        Unknown(document.Projector.Unknown, "projector.");

        var modules = new HashSet<ModuleId>();
        foreach (var slug in document.Modules)
        {
            if (ModuleIds.TryFromSlug(slug, out var id))
            {
                modules.Add(id);
            }
            else
            {
                errors.Add($"modules の {slug} というモジュールはありません ({string.Join(", ", ModuleIds.All.Select(m => m.Slug()))} のどれか)");
            }
        }

        if (modules.Contains(ModuleId.Camera) && !modules.Contains(ModuleId.BallTracker))
        {
            errors.Add("camera は adb でこの PC の ball-tracker につなぐので、modules に ball-tracker も入れてください");
        }

        if (document.MachineId < 0)
        {
            errors.Add("machine-id は 0 以上にしてください");
        }

        CollisionOutput? output = document.BallTracker.Output switch
        {
            "network" => CollisionOutput.Network,
            "csv" => CollisionOutput.Csv,
            _ => null,
        };
        if (output is null)
        {
            errors.Add($"ball-tracker.output は \"network\" か \"csv\" にしてください (今は \"{document.BallTracker.Output}\")");
        }

        var directory = Path.GetDirectoryName(Path.GetFullPath(path))!;
        var csvPath = Resolve(document.BallTracker.CsvPath);
        if (output == CollisionOutput.Csv && csvPath is null)
        {
            errors.Add("ball-tracker.output が \"csv\" のときは ball-tracker.csv-path も書いてください");
        }

        if (document.Projector.Monitor < 1)
        {
            errors.Add("projector.monitor は 1 以上にしてください");
        }

        if (errors.Count > 0)
        {
            throw new LauncherException($"{path} の設定が正しくありません:\n" + string.Join('\n', errors.Select(e => "・" + e)));
        }

        return new LauncherConfig(
            (uint)document.MachineId,
            modules,
            new Dictionary<ModuleId, string>
            {
                [ModuleId.BallTracker] = document.Hosts.BallTracker,
                [ModuleId.GameMaster] = document.Hosts.GameMaster,
            },
            new BallTrackerSettings(output!.Value, csvPath, document.BallTracker.ExtraArgs),
            new ProjectorSettings(document.Projector.Monitor, document.Projector.Fullscreen, Resolve(document.Projector.CollisionTransform)));

        void Unknown(Dictionary<string, object?> keys, string prefix) =>
            errors.AddRange(keys.Keys.Select(key => $"{prefix}{key} という設定はありません"));

        string? Resolve(string? value) =>
            string.IsNullOrWhiteSpace(value) ? null : Path.GetFullPath(value, directory);
    }

    // The shape of launcher.toml. LauncherConfigTemplate documents each key.
    private sealed class Document
    {
        public long MachineId { get; set; } = 1;

        public List<string> Modules { get; set; } = [];

        public HostsTable Hosts { get; set; } = new();

        public BallTrackerTable BallTracker { get; set; } = new();

        public ProjectorTable Projector { get; set; } = new();

        [TomlExtensionData]
        public Dictionary<string, object?> Unknown { get; set; } = [];
    }

    private sealed class HostsTable
    {
        public string BallTracker { get; set; } = "localhost";

        public string GameMaster { get; set; } = "localhost";

        [TomlExtensionData]
        public Dictionary<string, object?> Unknown { get; set; } = [];
    }

    private sealed class BallTrackerTable
    {
        public string Output { get; set; } = "network";

        public string? CsvPath { get; set; }

        public List<string> ExtraArgs { get; set; } = [];

        [TomlExtensionData]
        public Dictionary<string, object?> Unknown { get; set; } = [];
    }

    private sealed class ProjectorTable
    {
        public int Monitor { get; set; } = 2;

        public bool Fullscreen { get; set; } = true;

        public string? CollisionTransform { get; set; }

        [TomlExtensionData]
        public Dictionary<string, object?> Unknown { get; set; } = [];
    }
}
