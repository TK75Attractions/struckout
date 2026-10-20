using CommunityToolkit.Mvvm.ComponentModel;
using Struckout.Launcher.Core.Platform;
using Struckout.Launcher.Core.Spec;

namespace Struckout.Launcher.ViewModels;

public sealed partial class MainWindowViewModel : ObservableObject, IDisposable
{
    public MainWindowViewModel(HostPlatform platform, AppPaths paths, ServerPorts ports, HttpClient http, ICommandRunner runner)
    {
        Install = new InstallViewModel(platform, paths, http, runner);
        Launch = new LaunchViewModel(platform, paths, ports, runner);
        // The launch screen shows what's installed and the config the installer may have written.
        Install.Changed += (_, _) =>
        {
            if (Launch.CanReload)
            {
                Launch.Reload();
            }
        };
    }

    public InstallViewModel Install { get; }

    public LaunchViewModel Launch { get; }

    public async Task InitializeAsync()
    {
        Launch.Reload();
        await Install.InitializeAsync();
    }

    public void Dispose()
    {
        Install.Dispose();
        Launch.Dispose();
    }
}
