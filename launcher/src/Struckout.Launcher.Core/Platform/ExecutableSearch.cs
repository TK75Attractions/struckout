namespace Struckout.Launcher.Core.Platform;

internal static class ExecutableSearch
{
    /// <summary>Finds <paramref name="fileName"/> in the directories of <c>PATH</c>.</summary>
    public static string? OnPath(string fileName)
    {
        var path = Environment.GetEnvironmentVariable("PATH");
        if (string.IsNullOrEmpty(path))
        {
            return null;
        }

        return path.Split(Path.PathSeparator, StringSplitOptions.RemoveEmptyEntries)
            .Select(directory => Path.Combine(directory.Trim('"'), fileName))
            .FirstOrDefault(File.Exists);
    }

    /// <summary>The first of <paramref name="candidates"/> that exists.</summary>
    public static string? FirstExisting(IEnumerable<string?> candidates) =>
        candidates.FirstOrDefault(candidate => !string.IsNullOrEmpty(candidate) && File.Exists(candidate));
}
