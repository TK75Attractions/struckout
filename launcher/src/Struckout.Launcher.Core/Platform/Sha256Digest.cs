using System.Globalization;

namespace Struckout.Launcher.Core.Platform;

/// <summary>A SHA-256 hash of a file.</summary>
public sealed record Sha256Digest
{
    private const string GitHubPrefix = "sha256:";

    private Sha256Digest(string hex) => Hex = hex;

    /// <summary>64 lowercase hexadecimal digits.</summary>
    public string Hex { get; }

    public static Sha256Digest FromHash(ReadOnlySpan<byte> hash) =>
        hash.Length == 32
            ? new Sha256Digest(Convert.ToHexStringLower(hash))
            : throw new ArgumentException("a SHA-256 hash is 32 bytes", nameof(hash));

    /// <summary>
    /// Reads the <c>digest</c> GitHub reports for a release asset (<c>sha256:&lt;hex&gt;</c>).
    /// https://docs.github.com/rest/releases/assets
    /// </summary>
    /// <returns>Null when GitHub reports none, or a hash of another algorithm.</returns>
    /// <exception cref="FormatException">It says SHA-256 but the hash is malformed.</exception>
    public static Sha256Digest? ParseGitHub(string? value)
    {
        if (value is null || !value.StartsWith(GitHubPrefix, StringComparison.Ordinal))
        {
            return null;
        }

        var hex = value[GitHubPrefix.Length..];
        if (hex.Length != 64 || !hex.All(char.IsAsciiHexDigit))
        {
            throw new FormatException($"invalid SHA-256 digest: {value}");
        }

        return new Sha256Digest(hex.ToLower(CultureInfo.InvariantCulture));
    }

    public override string ToString() => GitHubPrefix + Hex;
}
