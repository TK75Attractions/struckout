using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Struckout.Launcher.Core;
using Struckout.Launcher.Core.GitHub;
using Struckout.Launcher.Core.Install;
using Struckout.Launcher.Core.Modules;
using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.ViewModels;

/// <summary>The install screen: choose modules and install, update or uninstall them.</summary>
public sealed partial class InstallViewModel : ObservableObject, IDisposable
{
    private readonly HostPlatform platform;
    private readonly AppPaths paths;
    private readonly HttpClient http;
    private readonly ICommandRunner runner;
    private Installer? installer;
    private Release? release;
    private CancellationTokenSource? operation;

    public InstallViewModel(HostPlatform platform, AppPaths paths, HttpClient http, ICommandRunner runner)
    {
        this.platform = platform;
        this.paths = paths;
        this.http = http;
        this.runner = runner;
        Modules = new ObservableCollection<ModuleItemViewModel>(ModuleIds.All.Select(id => new ModuleItemViewModel(id)));
    }

    /// <summary>Raised after modules were installed or uninstalled.</summary>
    public event EventHandler? Changed;

    public ObservableCollection<ModuleItemViewModel> Modules { get; }

    public LogViewModel Log { get; } = new();

    public string InstallDirectory => paths.DataDirectory;

    public string ConfigDirectory => paths.ConfigDirectory;

    [ObservableProperty]
    public partial string ReleaseText { get; private set; } = "リリースを確かめています…";

    [ObservableProperty]
    public partial string CredentialText { get; private set; } = "";

    [ObservableProperty]
    [NotifyCanExecuteChangedFor(nameof(InstallCommand), nameof(UninstallCommand), nameof(RefreshCommand), nameof(CancelCommand))]
    public partial bool IsBusy { get; private set; }

    [ObservableProperty]
    public partial bool DeleteDatabase { get; set; }

    [ObservableProperty]
    public partial double DownloadPercent { get; private set; }

    [ObservableProperty]
    public partial bool IsDownloading { get; private set; }

    public async Task InitializeAsync()
    {
        IsBusy = true;
        try
        {
            var credential = await GitHubCredential.DetectAsync(runner, CancellationToken.None);
            CredentialText = "GitHub: " + credential.Describe();
            installer = new Installer(
                platform, paths, new GitHubClient(http, GitHubClient.DefaultRepository, credential),
                new HttpDownloader(http), runner);
            UpdateModules();
            foreach (var module in Modules)
            {
                module.IsSelected = module.IsInstalled;
            }
        }
        catch (LauncherException e)
        {
            Log.Append(e.Message);
        }
        finally
        {
            IsBusy = false;
        }

        await RefreshAsync();
    }

    private bool CanRun() => !IsBusy && installer is not null;

    /// <summary>Fetches the release to show whether updates are available.</summary>
    [RelayCommand(CanExecute = nameof(CanRun))]
    private async Task RefreshAsync()
    {
        await RunAsync(async (installer, token) =>
        {
            release = await installer.GetReleaseAsync(GitHubClient.DefaultReleaseTag, token);
            ReleaseText = $"リリース {release.Tag} ({ModuleItemViewModel.FormatTime(release.PublishedAt)} 公開)";
        });
    }

    [RelayCommand(CanExecute = nameof(CanRun))]
    private async Task InstallAsync()
    {
        var selected = Modules.Where(m => m.IsSelected).Select(m => m.Id).ToList();
        if (selected.Count == 0)
        {
            Log.Append("インストールするモジュールを選んでください");
            return;
        }

        await RunAsync(async (installer, token) =>
        {
            release ??= await installer.GetReleaseAsync(GitHubClient.DefaultReleaseTag, token);
            var download = new Progress<DownloadProgress>(p =>
            {
                IsDownloading = p.Total is > 0 && p.Received < p.Total;
                DownloadPercent = p.Total is > 0 ? 100.0 * p.Received / p.Total.Value : 0;
            });
            Log.Append($"{string.Join("、", selected.Select(m => m.Slug()))} をインストールします");
            await installer.InstallAsync(selected, release, Log.CreateProgress(), download, token);
            Log.Append("インストールが終わりました");
        });
    }

    [RelayCommand(CanExecute = nameof(CanRun))]
    private async Task UninstallAsync()
    {
        var selected = Modules.Where(m => m.IsSelected && m.IsInstalled).Select(m => m.Id).ToList();
        if (selected.Count == 0)
        {
            Log.Append("アンインストールするモジュール (インストール済みのもの) を選んでください");
            return;
        }

        await RunAsync(async (installer, token) =>
        {
            Log.Append($"{string.Join("、", selected.Select(m => m.Slug()))} をアンインストールします");
            await installer.UninstallAsync(
                selected, new UninstallOptions { DeleteDatabase = DeleteDatabase }, Log.CreateProgress(), token);
            Log.Append($"アンインストールが終わりました (設定は {paths.ConfigDirectory} に残しています)");
        });
    }

    [RelayCommand(CanExecute = nameof(IsBusy))]
    private void Cancel() => operation?.Cancel();

    public void Dispose() => operation?.Dispose();

    private async Task RunAsync(Func<Installer, CancellationToken, Task> action)
    {
        if (installer is null)
        {
            return;
        }

        IsBusy = true;
        operation = new CancellationTokenSource();
        try
        {
            await action(installer, operation.Token);
        }
        catch (LauncherException e)
        {
            Log.Append(e.Message);
        }
        catch (OperationCanceledException)
        {
            Log.Append("中止しました");
        }
        finally
        {
            operation.Dispose();
            operation = null;
            IsDownloading = false;
            IsBusy = false;
            UpdateModules();
            Changed?.Invoke(this, EventArgs.Empty);
        }
    }

    private void UpdateModules()
    {
        foreach (var module in Modules)
        {
            module.Update(installer?.State.Modules.GetValueOrDefault(module.Id), release);
        }
    }
}
