using Avalonia.Controls;

namespace Struckout.Launcher.Views;

public sealed partial class InstallView : UserControl
{
    public InstallView()
    {
        InitializeComponent();
        LogBox.FollowEnd();
    }
}
