using Avalonia.Controls;

namespace Struckout.Launcher.Views;

public sealed partial class LaunchView : UserControl
{
    public LaunchView()
    {
        InitializeComponent();
        LogBox.FollowEnd();
    }
}
