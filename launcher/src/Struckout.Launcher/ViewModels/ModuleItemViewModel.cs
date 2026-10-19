using System.Globalization;
using CommunityToolkit.Mvvm.ComponentModel;
using Struckout.Launcher.Core.GitHub;
using Struckout.Launcher.Core.Install;
using Struckout.Launcher.Core.Modules;

namespace Struckout.Launcher.ViewModels;

/// <summary>A row of the install screen.</summary>
public sealed partial class ModuleItemViewModel(ModuleId id) : ObservableObject
{
    public ModuleId Id { get; } = id;

    public string Name => Id.Slug();

    public string Description => Id.Description();

    public string Prerequisites
    {
        get
        {
            var names = PrerequisiteInstaller.RequiredBy(Id).Select(PrerequisiteInstaller.DisplayName).ToList();
            return names.Count == 0 ? "" : "要るもの: " + string.Join("、", names);
        }
    }

    [ObservableProperty]
    public partial bool IsSelected { get; set; }

    [ObservableProperty]
    public partial bool IsInstalled { get; private set; }

    [ObservableProperty]
    public partial string Status { get; private set; } = "";

    [ObservableProperty]
    public partial bool HasUpdate { get; private set; }

    public void Update(ModuleInstallation? installation, Release? release)
    {
        IsInstalled = installation is not null;
        HasUpdate = installation is not null && release is not null && installation.ReleasePublishedAt < release.PublishedAt;
        Status = installation switch
        {
            null => "未インストール",
            _ when HasUpdate => $"更新あり ({FormatTime(installation.ReleasePublishedAt)} の版)",
            _ => $"インストール済み ({installation.ReleaseTag}、{FormatTime(installation.ReleasePublishedAt)} の版)",
        };
    }

    internal static string FormatTime(DateTimeOffset time) =>
        time.ToLocalTime().ToString("yyyy-MM-dd HH:mm", CultureInfo.InvariantCulture);
}
