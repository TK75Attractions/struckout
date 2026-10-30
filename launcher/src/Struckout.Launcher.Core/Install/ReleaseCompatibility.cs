using Struckout.Launcher.Core.Modules;

namespace Struckout.Launcher.Core.Install;

/// <summary>Checks whether the installed modules on this PC come from the same release.</summary>
public static class ReleaseCompatibility
{
    /// <summary>The modules whose release tags are compared.</summary>
    public static IEnumerable<ModuleId> Checked(IEnumerable<ModuleId> modules, InstallState state) => modules
        // Camera tags may describe a newly installed APK even when existing apps are still in use.
        .Where(id => id != ModuleId.Camera)
        .Where(state.Modules.ContainsKey);

    /// <summary>Returns an actionable message when recorded local module releases differ.</summary>
    public static string? FindMismatch(IEnumerable<ModuleId> modules, InstallState state)
    {
        var installations = Checked(modules, state)
            .Select(id => (Id: id, Tag: state.Modules[id].ReleaseTag))
            .ToList();

        if (installations.Select(installation => installation.Tag).Distinct(StringComparer.Ordinal).Count() <= 1)
        {
            return null;
        }

        var versions = string.Join("、", installations.Select(installation =>
            $"{installation.Id.Slug()} は {installation.Tag}"));
        return $"この PC のモジュールのリリースが一致しません: {versions}。インストール画面で「更新」して同じリリースにそろえてください";
    }
}
