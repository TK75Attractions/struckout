namespace Struckout.Launcher.Core.Modules;

/// <summary>A part of struckout that the launcher installs and starts.</summary>
public enum ModuleId
{
    BallTracker,
    Camera,
    GameMaster,
    Projector,
    Touchpanel,
}

public static class ModuleIds
{
    public static IReadOnlyList<ModuleId> All { get; } = Enum.GetValues<ModuleId>();

    /// <summary>
    /// The name used in the settings, the install state and the release assets
    /// (<c>ball-tracker-x86_64-windows</c>, ...).
    /// </summary>
    public static string Slug(this ModuleId id) => id switch
    {
        ModuleId.BallTracker => "ball-tracker",
        ModuleId.Camera => "camera",
        ModuleId.GameMaster => "game-master",
        ModuleId.Projector => "projector",
        ModuleId.Touchpanel => "touchpanel",
        _ => throw new ArgumentOutOfRangeException(nameof(id), id, null),
    };

    public static bool TryFromSlug(string slug, out ModuleId id)
    {
        foreach (var candidate in All)
        {
            if (candidate.Slug() == slug)
            {
                id = candidate;
                return true;
            }
        }

        id = default;
        return false;
    }

    /// <summary>What the module does, for the UI.</summary>
    public static string Description(this ModuleId id) => id switch
    {
        ModuleId.BallTracker => "カメラの検出から着弾位置を求め、projector に送る",
        ModuleId.Camera => "スマホのカメラアプリ (adb でつないだ端末に入れる)",
        ModuleId.GameMaster => "ゲームと得点、ランキングを管理するサーバー (Docker)",
        ModuleId.Projector => "的を描き、当たり判定をする",
        ModuleId.Touchpanel => "受付のタッチパネル",
        _ => throw new ArgumentOutOfRangeException(nameof(id), id, null),
    };
}
