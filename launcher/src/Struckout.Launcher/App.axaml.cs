using Avalonia;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Markup.Xaml;
using Struckout.Launcher.Core.GitHub;
using Struckout.Launcher.Core.Install;
using Struckout.Launcher.Core.Platform;
using Struckout.Launcher.Core.Spec;
using Struckout.Launcher.ViewModels;
using Struckout.Launcher.Views;

namespace Struckout.Launcher;

public sealed partial class App : Application, IDisposable
{
    private readonly HttpClient http = CreateHttpClient();

    public override void Initialize() => AvaloniaXamlLoader.Load(this);

    public override void OnFrameworkInitializationCompleted()
    {
        if (ApplicationLifetime is IClassicDesktopStyleApplicationLifetime desktop)
        {
            var platform = HostPlatform.Current;
            var paths = AppPaths.ForCurrentUser(platform);
            var runner = new ProcessCommandRunner();
            desktop.MainWindow = new MainWindow
            {
                DataContext = new MainWindowViewModel(platform, paths, ServerPorts.Spec, http, runner),
            };
            desktop.Exit += (_, _) => Dispose();
        }

        base.OnFrameworkInitializationCompleted();
    }

    public void Dispose() => http.Dispose();

    private static HttpClient CreateHttpClient()
    {
        var client = new HttpClient();
        // GitHub rejects API requests without a User-Agent
        client.DefaultRequestHeaders.UserAgent.ParseAdd("struckout-launcher");
        return client;
    }
}
