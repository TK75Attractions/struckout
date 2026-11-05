namespace Struckout.Launcher.Core.Tests;

/// <summary>Collects the reports synchronously, unlike <see cref="Progress{T}"/>.</summary>
internal sealed class ListLog : IProgress<string>
{
    private readonly List<string> lines = [];

    public IReadOnlyList<string> Lines
    {
        get
        {
            lock (lines)
            {
                return lines.ToList();
            }
        }
    }

    public void Report(string value)
    {
        lock (lines)
        {
            lines.Add(value);
        }
    }
}
