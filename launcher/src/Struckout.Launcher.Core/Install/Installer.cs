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

    /// <exception cref="LauncherException"></exception>
    public Task<Release> GetReleaseAsync(string tag, CancellationToken cancellationToken) =>
        github.GetReleaseAsync(tag, cancellationToken);

    /// <summary>
    /// Installs <paramref name="modules"/> from <paramref name="release"/>, with their prerequisites.
    /// Modules that are already installed are updated. The settings the user edited are kept.
    /// </summary>
    /// <exception cref="LauncherException">A step failed. The modules installed before it stay recorded.</exception>
    public async Task InstallAsync(
        IReadOnlyCollection<ModuleId> modules,
        Release release,
        IProgress<string> log,
        IProgress<DownloadProgress>? download,
        CancellationToken cancellationToken)
    {
        foreach (var id in ModuleIds.All.Where(modules.Contains))
        {
            var tools = new Dictionary<PrerequisiteId, string>();
            try
            {
                foreach (var prerequisite in PrerequisiteInstaller.RequiredBy(id))
                {
                    tools[prerequisite] = await prerequisites.EnsureAsync(prerequisite, State, log, cancellationToken);
                    State.Save(paths.InstallStateFile);
                }

                if (id is ModuleId.BallTracker or ModuleId.Camera)
                {
                    await EnsureCamerasFileAsync(release, log, cancellationToken);
                }

                var context = new InstallContext(release, tools, log, download, cancellationToken);
                State.Modules[id] = await installers[id].InstallAsync(context, State.Modules.GetValueOrDefault(id));
                State.Save(paths.InstallStateFile);
                log.Report($"{id.Slug()}: インストールしました");
            }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException)
            {
                throw new LauncherException(
                    $"{id.Slug()}: ファイルを書き込めません (起動中なら止めてからやり直してください): {e.Message}", e);
            }
        }

        if (LauncherConfigTemplate.WriteIfMissing(paths.LauncherConfigFile, State.Modules.Keys))
        {
            log.Report($"起動の設定を {paths.LauncherConfigFile} に作りました");
        }
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
