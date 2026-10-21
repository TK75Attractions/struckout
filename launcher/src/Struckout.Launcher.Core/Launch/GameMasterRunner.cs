using Struckout.Launcher.Core.Modules;

namespace Struckout.Launcher.Core.Launch;

/// <summary>game_master and its database, as compose services on this PC's Docker.</summary>
internal sealed class GameMasterRunner(GameMasterCompose compose, int grpcPort) : IModuleRunner
{
    public ModuleId Id => ModuleId.GameMaster;

    public async Task StartAsync(IProgress<string> log, CancellationToken cancellationToken)
    {
        await compose.EnsureEngineRunningAsync(log, cancellationToken);
        log.Report("game-master: コンテナを起動します");
        var result = await compose.RunAsync(["up", "--detach"], cancellationToken);
        if (!result.Succeeded)
        {
            throw new LauncherException($"docker compose up が失敗しました:\n{result.Output.Trim()}");
        }
    }

    public Task<Health> CheckAsync(CancellationToken cancellationToken) =>
        GrpcHealthProbe.CheckAsync("127.0.0.1", grpcPort, cancellationToken);

    public async Task StopAsync(IProgress<string> log, CancellationToken cancellationToken)
    {
        log.Report("game-master: コンテナを止めます");
        var result = await compose.RunAsync(["stop"], cancellationToken);
        if (!result.Succeeded)
        {
            throw new LauncherException($"docker compose stop が失敗しました:\n{result.Output.Trim()}");
        }
    }
}
