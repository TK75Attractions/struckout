using Struckout.Launcher.Core.GitHub;
using Struckout.Launcher.Core.Launch;
using Struckout.Launcher.Core.Modules;
using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.Core.Install;

/// <summary>Installs and uninstalls the modules, and records what it did in <see cref="InstallState"/>.</summary>
public sealed class Installer
{
    private readonly AppPaths paths;
    private readonly GitHubClient github;
    private readonly ToolLocator locator;
    private readonly PrerequisiteInstaller prerequisites;
    private readonly Dictionary<ModuleId, IModuleInstaller> installers;

    public Installer(HostPlatform platform, AppPaths paths, GitHubClient github, HttpDownloader downloader, ICommandRunner runner)
    {
        this.paths = paths;
        this.github = github;
        locator = new ToolLocator(platform, paths);
        prerequisites = new PrerequisiteInstaller(platform, paths, locator, runner, downloader);
        installers = new Dictionary<ModuleId, IModuleInstaller>
        {
            [ModuleId.BallTracker] = new ExecutableModuleInstaller(ModuleId.BallTracker, "ball_tracker", platform, paths, downloader),
            [ModuleId.Touchpanel] = new ExecutableModuleInstaller(ModuleId.Touchpanel, "touchpanel", platform, paths, downloader),
            [ModuleId.Projector] = new ProjectorInstaller(platform, paths, downloader),
            [ModuleId.GameMaster] = new GameMasterInstaller(platform, paths, github, runner),
            [ModuleId.Camera] = new CameraInstaller(paths, downloader, runner),
        };
        State = InstallState.Load(paths.InstallStateFile);
    }

    public InstallState State { get; }

    /// <summary>The release to install: the latest one.</summary>
    /// <exception cref="LauncherException">There is no release or GitHub can't be reached.</exception>
    public Task<Release> GetReleaseAsync(CancellationToken cancellationToken) =>
        github.GetLatestReleaseAsync(cancellationToken);

    /// <summary>
    /// Installs <paramref name="modules"/> from <paramref name="release"/>, with their prerequisites.
    /// Modules that are already installed are updated. The settings the user edited are kept.
    /// </summary>
    /// <remarks>
    /// The prerequisites are installed one at a time, because winget and Homebrew refuse to run twice at once.
    /// The modules are then installed at the same time: they write to their own directories, and most of the time
    /// goes to downloading them (projector alone is about 50 MB).
    /// </remarks>
    /// <exception cref="LauncherException">
    /// A prerequisite or a module failed. The other modules are still installed and recorded.
    /// </exception>
    public async Task InstallAsync(
        IReadOnlyCollection<ModuleId> modules,
        Release release,
        IProgress<string> log,
        IProgress<DownloadProgress>? download,
        CancellationToken cancellationToken)
    {
        var selected = ModuleIds.All.Where(modules.Contains).ToList();
        var tools = new Dictionary<PrerequisiteId, string>();
        foreach (var prerequisite in selected.SelectMany(PrerequisiteInstaller.RequiredBy).Distinct())
        {
            tools[prerequisite] = await prerequisites.EnsureAsync(prerequisite, State, log, cancellationToken);
            State.Save(paths.InstallStateFile);
        }

        if (selected.Any(id => id is ModuleId.BallTracker or ModuleId.Camera))
        {
            await EnsureCamerasFileAsync(release, log, cancellationToken);
        }

        var downloads = new CombinedDownloadProgress(download);
        var failures = await Task.WhenAll(selected.Select(id =>
        {
            var context = new InstallContext(
                release,
                PrerequisiteInstaller.RequiredBy(id).ToDictionary(p => p, p => tools[p]),
                log,
                downloads.Add(),
                cancellationToken);
            return InstallModuleAsync(id, context);
        }));

        if (LauncherConfigTemplate.WriteIfMissing(paths.LauncherConfigFile, State.Modules.Keys))
        {
            log.Report($"起動の設定を {paths.LauncherConfigFile} に作りました");
        }

        var messages = failures.OfType<string>().ToList();
        if (messages.Count > 0)
        {
            throw new LauncherException("インストールできなかったモジュールがあります:\n" + string.Join('\n', messages));
        }
    }

    /// <returns>Why it failed, or null when it's installed.</returns>
    private async Task<string?> InstallModuleAsync(ModuleId id, InstallContext context)
    {
        ModuleInstallation installation;
        try
        {
            ModuleInstallation? previous;
            lock (State)
            {
                previous = State.Modules.GetValueOrDefault(id);
            }

            installation = await installers[id].InstallAsync(context, previous);
        }
        catch (LauncherException e)
        {
            return $"・{id.Slug()}: {e.Message}";
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            return $"・{id.Slug()}: ファイルを書き込めません (起動中なら止めてからやり直してください): {e.Message}";
        }

        lock (State)
        {
            State.Modules[id] = installation;
            State.Save(paths.InstallStateFile);
        }

        context.Log.Report($"{id.Slug()}: インストールしました");
        return null;
    }

    /// <summary>
    /// Uninstalls <paramref name="modules"/>, then the prerequisites we installed that no remaining module needs.
    /// The settings in <see cref="AppPaths.ConfigDirectory"/> are kept.
    /// </summary>
    /// <exception cref="LauncherException">A step failed.</exception>
    public async Task UninstallAsync(
        IReadOnlyCollection<ModuleId> modules, UninstallOptions options, IProgress<string> log, CancellationToken cancellationToken)
    {
        foreach (var id in ModuleIds.All.Where(modules.Contains))
        {
            if (!State.Modules.TryGetValue(id, out var installation))
            {
                continue;
            }

            var tools = PrerequisiteInstaller.RequiredBy(id)
                .Select(p => (Id: p, Path: locator.Find(p)))
                .Where(t => t.Path is not null)
                .ToDictionary(t => t.Id, t => t.Path!);
            try
            {
                await installers[id].UninstallAsync(installation, tools, options, log, cancellationToken);
            }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException)
            {
                throw new LauncherException(
                    $"{id.Slug()}: ファイルを消せません (起動中なら止めてからやり直してください): {e.Message}", e);
            }

            State.Modules.Remove(id);
            State.Save(paths.InstallStateFile);
            log.Report($"{id.Slug()}: アンインストールしました");
        }

        var stillNeeded = State.Modules.Keys.SelectMany(PrerequisiteInstaller.RequiredBy).ToHashSet();
        foreach (var (id, origin) in State.Prerequisites.Where(p => !stillNeeded.Contains(p.Key)).ToList())
        {
            await prerequisites.RemoveAsync(id, origin, log, cancellationToken);
            State.Prerequisites.Remove(id);
            State.Save(paths.InstallStateFile);
        }
    }

    /// <summary>Copies <c>scripts/cameras.json</c> of the release unless the user already has one.</summary>
    private async Task EnsureCamerasFileAsync(Release release, IProgress<string> log, CancellationToken cancellationToken)
    {
        if (File.Exists(paths.CamerasFile))
        {
            return;
        }

        log.Report($"{CamerasFile.RepositoryPath} を {paths.CamerasFile} にコピーします");
        var content = await github.GetRawFileAsync(release.Tag, CamerasFile.RepositoryPath, cancellationToken);
        Directory.CreateDirectory(paths.ConfigDirectory);
        await File.WriteAllTextAsync(paths.CamerasFile, content, cancellationToken);
    }
}
