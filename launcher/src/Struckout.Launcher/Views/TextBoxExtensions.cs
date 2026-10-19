using Avalonia.Controls;

namespace Struckout.Launcher.Views;

internal static class TextBoxExtensions
{
    /// <summary>Keeps the newest line of a log in view.</summary>
    public static void FollowEnd(this TextBox box) =>
        box.PropertyChanged += (_, e) =>
        {
            if (e.Property == TextBox.TextProperty)
            {
                box.CaretIndex = box.Text?.Length ?? 0;
            }
        };
}
