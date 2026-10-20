using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.Core.GitHub;

public enum CredentialSource
{
    /// <summary>No token: the API allows 60 requests an hour per IP address.</summary>
    Anonymous,
    Environment,
    GitHubCli,
}

/// <summary>A token for api.github.com, if the user has one.</summary>
/// <remarks>
/// Everything the launcher downloads is public, so the token only raises the API rate limit
/// (several PCs at a booth share one IP address). We don't embed a token of our own: anyone could
/// extract it from the binary.
/// </remarks>
public sealed record GitHubCredential(CredentialSource Source, string? Token)
{
    public static GitHubCredential Anonymous { get; } = new(CredentialSource.Anonymous, null);

    /// <summary>
    /// Takes <c>GH_TOKEN</c> or <c>GITHUB_TOKEN</c> (the same precedence as gh), then <c>gh auth token</c>.
    /// </summary>
    public static async Task<GitHubCredential> DetectAsync(ICommandRunner runner, CancellationToken cancellationToken)
    {
        foreach (var name in new[] { "GH_TOKEN", "GITHUB_TOKEN" })
        {
            var value = Environment.GetEnvironmentVariable(name);
            if (!string.IsNullOrWhiteSpace(value))
            {
                return new GitHubCredential(CredentialSource.Environment, value.Trim());
            }
        }

        var gh = ExecutableSearch.OnPath(HostPlatform.Current.ExecutableName("gh"));
        if (gh is null)
        {
            return Anonymous;
        }

        var result = await runner.RunAsync(gh, ["auth", "token"], cancellationToken: cancellationToken);
        var token = result.Output.Trim();
        // gh exits with 1 when it isn't logged in
        return result.Succeeded && token.Length > 0 && !token.Contains('\n', StringComparison.Ordinal)
            ? new GitHubCredential(CredentialSource.GitHubCli, token)
            : Anonymous;
    }

    public string Describe() => Source switch
    {
        CredentialSource.Anonymous => "未認証 (GitHub API は 1 時間に 60 回まで)",
        CredentialSource.Environment => "環境変数のトークン",
        CredentialSource.GitHubCli => "GitHub CLI (gh auth token)",
        _ => throw new InvalidOperationException($"unknown source {Source}"),
    };
}
