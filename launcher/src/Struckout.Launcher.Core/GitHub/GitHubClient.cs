using System.Net;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using System.Text.Json;
using System.Text.Json.Serialization;
using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.Core.GitHub;

/// <param name="Digest">The hash GitHub computed when the asset was uploaded. Null for old assets.</param>
public sealed record ReleaseAsset(string Name, Uri DownloadUrl, long Size, Sha256Digest? Digest);

public sealed record Release(string Tag, DateTimeOffset PublishedAt, IReadOnlyList<ReleaseAsset> Assets)
{
    public ReleaseAsset? FindAsset(string name) => Assets.FirstOrDefault(a => a.Name == name);
}

/// <summary>Reads releases and files of the struckout repository.</summary>
public sealed class GitHubClient(HttpClient http, string repository, GitHubCredential credential)
{
    /// <summary>The repository the releases come from.</summary>
    public const string DefaultRepository = "TK75Attractions/struckout";

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
    };

    public string Repository { get; } = repository;

    public GitHubCredential Credential { get; } = credential;

    /// <summary>
    /// The newest release that <c>.github/workflows/release.yaml</c> published (tagged like <c>2026-10-19.0</c>).
    /// Pre-releases such as the old <c>nightly</c> are skipped.
    /// </summary>
    /// <exception cref="LauncherException">There is no release or GitHub can't be reached.</exception>
    public Task<Release> GetLatestReleaseAsync(CancellationToken cancellationToken) =>
        GetReleaseAsync("releases/latest", "最新のリリース", cancellationToken);

    /// <exception cref="LauncherException">The release doesn't exist or GitHub can't be reached.</exception>
    public Task<Release> GetReleaseAsync(string tag, CancellationToken cancellationToken) =>
        GetReleaseAsync($"releases/tags/{Uri.EscapeDataString(tag)}", $"リリース {tag}", cancellationToken);

    private async Task<Release> GetReleaseAsync(string path, string name, CancellationToken cancellationToken)
    {
        using var request = new HttpRequestMessage(HttpMethod.Get, $"https://api.github.com/repos/{Repository}/{path}");
        request.Headers.Accept.Add(new MediaTypeWithQualityHeaderValue("application/vnd.github+json"));
        request.Headers.Add("X-GitHub-Api-Version", "2022-11-28");
        // Only api.github.com gets the token. HttpClient drops it on redirects to other hosts.
        if (Credential.Token is { } token)
        {
            request.Headers.Authorization = new AuthenticationHeaderValue("Bearer", token);
        }

        try
        {
            using var response = await http.SendAsync(request, cancellationToken);
            if (response.StatusCode == HttpStatusCode.NotFound)
            {
                throw new LauncherException($"{Repository} に{name}がありません");
            }

            if (response.StatusCode is HttpStatusCode.Forbidden or HttpStatusCode.TooManyRequests)
            {
                throw new LauncherException(
                    "GitHub API の回数制限に達しました。GitHub CLI でログインする (gh auth login) か、"
                    + "しばらく待ってからやり直してください");
            }

            response.EnsureSuccessStatusCode();
            var dto = await response.Content.ReadFromJsonAsync<ReleaseDto>(JsonOptions, cancellationToken)
                ?? throw new LauncherException("GitHub が空のリリース情報を返しました");
            return new Release(
                dto.TagName,
                dto.PublishedAt,
                dto.Assets
                    .Select(a => new ReleaseAsset(
                        a.Name, new Uri(a.BrowserDownloadUrl), a.Size, Sha256Digest.ParseGitHub(a.Digest)))
                    .ToList());
        }
        catch (HttpRequestException e)
        {
            throw new LauncherException($"GitHub からリリースの情報を取れません: {e.Message}", e);
        }
        catch (Exception e) when (e is JsonException or FormatException)
        {
            throw new LauncherException($"GitHub のリリースの情報を読めません: {e.Message}", e);
        }
    }

    /// <summary>A file of the repository at <paramref name="gitRef"/> (a tag, a branch or a commit).</summary>
    /// <exception cref="LauncherException">The file doesn't exist or GitHub can't be reached.</exception>
    public async Task<string> GetRawFileAsync(string gitRef, string path, CancellationToken cancellationToken)
    {
        var url = $"https://raw.githubusercontent.com/{Repository}/{Uri.EscapeDataString(gitRef)}/{path}";
        try
        {
            using var response = await http.GetAsync(url, cancellationToken);
            if (response.StatusCode == HttpStatusCode.NotFound)
            {
                throw new LauncherException($"{Repository} の {gitRef} に {path} がありません");
            }

            response.EnsureSuccessStatusCode();
            return await response.Content.ReadAsStringAsync(cancellationToken);
        }
        catch (HttpRequestException e)
        {
            throw new LauncherException($"{url} を取れません: {e.Message}", e);
        }
    }

    private sealed record ReleaseDto(
        [property: JsonRequired] string TagName,
        [property: JsonRequired] DateTimeOffset PublishedAt,
        [property: JsonRequired] IReadOnlyList<AssetDto> Assets);

    private sealed record AssetDto(
        [property: JsonRequired] string Name,
        [property: JsonRequired] string BrowserDownloadUrl,
        [property: JsonRequired] long Size,
        string? Digest);
}
