using Avalonia.Controls;
using Struckout.Launcher.ViewModels;

namespace Struckout.Launcher.Views;

public sealed partial class MainWindow : Window
{
    private bool shutDown;

    public MainWindow()
    {
        InitializeComponent();
        Opened += async (_, _) =>
        {
            if (DataContext is MainWindowViewModel viewModel)
            {
                await viewModel.InitializeAsync();
            }
        };
        Closing += OnClosing;
    }

    /// <summary>Stops the modules before closing, so that no process is left without the launcher.</summary>
    private async void OnClosing(object? sender, WindowClosingEventArgs e)
    {
        if (shutDown || DataContext is not MainWindowViewModel viewModel)
        {
            return;
        }

        e.Cancel = true;
        IsEnabled = false;
        await viewModel.Launch.ShutdownAsync();
        viewModel.Dispose();
        shutDown = true;
        Close();
    }
}
