using Struckout.Launcher.Core.GitHub;
using Struckout.Launcher.Core.Modules;
using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.Core.Install;

/// <summary>game_master: the compose files of the release and the image on ghcr.io.</summary>
internal sealed class GameMasterInstaller(
    HostPlatform platform, AppPaths paths, GitHubClient github, ICommandRunner runner) : IModuleInstaller
{
    public async Task<ModuleInstallation> InstallAsync(InstallContext context, ModuleInstallation? previous)
    {
        var slug = ModuleId.GameMaster.Slug();
        var directory = paths.ModuleDirectory(slug);
        Directory.CreateDirectory(directory);
        foreach (var file in GameMasterCompose.Files)
        {
            context.Log.Report($"{slug}: game_master/{file} を取得します");
            var content = await github.GetRawFileAsync(context.Release.Tag, $"game_master/{file}", context.CancellationToken);
            await File.WriteAllTextAsync(Path.Combine(directory, file), content, context.CancellationToken);
        }

        var compose = new GameMasterCompose(
            platform, context.Tools[PrerequisiteId.Docker], directory, context.Release.Tag,
            GameMasterSecret.LoadOrCreate(paths), runner);
        await compose.EnsureEngineRunningAsync(context.Log, context.CancellationToken);
        context.Log.Report($"{slug}: Docker のイメージを取得します (初回は数分かかります)");
        var result = await compose.RunAsync(["pull", "--quiet"], context.CancellationToken);
        if (!result.Succeeded)
        {
            throw new LauncherException(
                "game_master のイメージを取得できません。ghcr.io のパッケージが公開されているか確かめてください:\n"
                + result.Output.Trim());
        }

        return new ComposeInstallation
        {
            ReleaseTag = context.Release.Tag,
            ReleasePublishedAt = context.Release.PublishedAt,
            Directory = directory,
        };
    }

    public async Task UninstallAsync(
        ModuleInstallation installation,
        IReadOnlyDictionary<PrerequisiteId, string> tools,
        UninstallOptions options,
        IProgress<string> log,
        CancellationToken cancellationToken)
    {
        var slug = ModuleId.GameMaster.Slug();
        if (tools.TryGetValue(PrerequisiteId.Docker, out var docker) && Directory.Exists(installation.Directory))
        {
            var compose = new GameMasterCompose(
                platform, docker, installation.Directory, installation.ReleaseTag, GameMasterSecret.Load(paths) ?? "",
                runner);
            await compose.EnsureEngineRunningAsync(log, cancellationToken);
            log.Report(options.DeleteDatabase
                ? $"{slug}: コンテナとイメージ、データベースを消します"
                : $"{slug}: コンテナとイメージを消します (データベースは残します)");
            var result = await compose.RunAsync(
                options.DeleteDatabase ? ["down", "--rmi", "all", "--volumes"] : ["down", "--rmi", "all"],
                cancellationToken);
            if (!result.Succeeded)
            {
                throw new LauncherException($"docker compose down が失敗しました:\n{result.Output.Trim()}");
            }
        }
        else
        {
            log.Report($"{slug}: Docker が無いので、コンテナとイメージは消しません");
        }

        if (options.DeleteDatabase)
        {
            GameMasterSecret.Delete(paths);
        }

        if (Directory.Exists(installation.Directory))
        {
            Directory.Delete(installation.Directory, recursive: true);
        }
    }
}
