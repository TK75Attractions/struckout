using System.Globalization;
using CommunityToolkit.Mvvm.ComponentModel;

namespace Struckout.Launcher.ViewModels;

/// <summary>The latest lines of what the installer or the launcher did.</summary>
public sealed partial class LogViewModel : ObservableObject
{
    private const int MaxLines = 1000;

    private readonly Queue<string> lines = new();

    [ObservableProperty]
    public partial string Text { get; private set; } = "";

    /// <summary>Must be called on the UI thread (<see cref="Progress{T}"/> takes care of it).</summary>
    public void Append(string message)
    {
        var time = DateTime.Now.ToString("HH:mm:ss", CultureInfo.InvariantCulture);
        foreach (var line in message.Split('\n'))
        {
            lines.Enqueue($"{time}  {line.TrimEnd('\r')}");
        }

        while (lines.Count > MaxLines)
        {
            lines.Dequeue();
        }

        Text = string.Join('\n', lines);
    }

    public IProgress<string> CreateProgress() => new Progress<string>(Append);
}
