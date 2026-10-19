using System.Globalization;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace Struckout.Launcher.Core.Modules;

/// <summary>A phone in <c>cameras.json</c>.</summary>
/// <param name="Area">The detection area, if it's limited.</param>
public sealed record CameraEntry(
    string Serial,
    string? DeviceId,
    string Name,
    string Output,
    double FocalRatio,
    IReadOnlyList<double> Pose,
    IReadOnlyList<double>? Area);

/// <summary>
/// The per-phone settings (<c>scripts/cameras.json</c> in the repository). ball_tracker reads it with
/// <c>--cameras</c>, and the launcher sends it to the phones.
/// </summary>
public sealed class CamerasFile
{
    /// <summary>Where the installer takes the first copy from.</summary>
    public const string RepositoryPath = "scripts/cameras.json";

    private readonly string path;
    private readonly JsonObject root;

    private CamerasFile(string path, JsonObject root)
    {
        this.path = path;
        this.root = root;
        Package = root["package"]?.GetValue<string>()
            ?? throw new LauncherException($"{path} に package がありません");
        Cameras = (root["cameras"] as JsonArray ?? throw new LauncherException($"{path} に cameras がありません"))
            .Select(node => ParseCamera(node as JsonObject ?? throw new LauncherException($"{path} の cameras が壊れています")))
            .ToList();
    }

    /// <summary>The application ID of the camera app.</summary>
    public string Package { get; }

    /// <summary>In the order ball_tracker assigns camera IDs (0, 1, ...).</summary>
    public IReadOnlyList<CameraEntry> Cameras { get; private set; }

    /// <exception cref="LauncherException">The file is missing or broken.</exception>
    public static CamerasFile Load(string path)
    {
        try
        {
            // PowerShell writes a BOM, which JsonNode rejects
            var text = File.ReadAllText(path).TrimStart('﻿');
            return new CamerasFile(
                path,
                JsonNode.Parse(text) as JsonObject ?? throw new LauncherException($"{path} が JSON のオブジェクトではありません"));
        }
        catch (Exception e) when (e is FileNotFoundException or DirectoryNotFoundException)
        {
            throw new LauncherException($"{path} がありません。インストールし直してください", e);
        }
        catch (JsonException e)
        {
            throw new LauncherException($"{path} が壊れています: {e.Message}", e);
        }
        catch (InvalidOperationException e)
        {
            // GetValue<T> on a value of another type
            throw new LauncherException($"{path} が壊れています: {e.Message}", e);
        }
    }

    /// <summary>
    /// Records the ID the phone reported, by which ball_tracker tells the phones apart.
    /// The rest of the file is kept as it is.
    /// </summary>
    /// <returns>Whether the file changed.</returns>
    public bool SetDeviceId(string serial, string deviceId)
    {
        var camera = (root["cameras"] as JsonArray)!
            .OfType<JsonObject>()
            .First(c => c["serial"]?.GetValue<string>() == serial);
        if (camera["device_id"]?.GetValue<string>() == deviceId)
        {
            return false;
        }

        camera["device_id"] = deviceId;
        File.WriteAllText(path, root.ToJsonString(new JsonSerializerOptions { WriteIndented = true }) + "\n");
        Cameras = Cameras.Select(c => c.Serial == serial ? c with { DeviceId = deviceId } : c).ToList();
        return true;
    }

    private CameraEntry ParseCamera(JsonObject camera)
    {
        string Required(string key) =>
            camera[key]?.GetValue<string>() ?? throw new LauncherException($"{path} のカメラに {key} がありません");

        IReadOnlyList<double>? Numbers(string key) =>
            (camera[key] as JsonArray)?.Select(n => n!.GetValue<double>()).ToList();

        return new CameraEntry(
            Required("serial"),
            camera["device_id"]?.GetValue<string>(),
            Required("name"),
            Required("output"),
            camera["focal_ratio"]?.GetValue<double>() ?? throw new LauncherException($"{path} のカメラに focal_ratio がありません"),
            Numbers("pose") ?? throw new LauncherException($"{path} のカメラに pose がありません"),
            Numbers("area"));
    }

    /// <summary>The numbers joined by commas, as the camera app's remote control expects.</summary>
    public static string JoinNumbers(IEnumerable<double> values) =>
        string.Join(',', values.Select(v => v.ToString(CultureInfo.InvariantCulture)));
}
