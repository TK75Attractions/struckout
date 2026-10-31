namespace Struckout.Launcher.Core.Platform;

/// <summary>Where the launcher keeps its settings and the modules it installs.</summary>
/// <param name="ConfigDirectory">
/// Settings the user edits (<c>launcher.toml</c>, <c>cameras.json</c>). The base directory is the same as
/// Rust's <c>dirs::config_dir()</c>, which ball_tracker uses, so every module's settings sit under one
/// <c>struckout</c> directory (<c>%APPDATA%</c> on Windows).
/// </param>
/// <param name="DataDirectory">
/// Downloaded modules, tools, logs and the install state. The base directory is the same as
/// <c>dirs::data_local_dir()</c>: binaries should not roam with a Windows profile.
/// </param>
public sealed record AppPaths(string ConfigDirectory, string DataDirectory)
{
    public string LauncherConfigFile => Path.Combine(ConfigDirectory, "launcher.toml");

    /// <summary>The file touchpanel reads from Rust's <c>dirs::config_dir()</c>.</summary>
    public string TouchpanelConfigFile => Path.Combine(Path.GetDirectoryName(ConfigDirectory)!, "touchpanel", "config.toml");

    /// <summary>Per-phone settings shared by ball_tracker (<c>--cameras</c>) and the camera app.</summary>
    public string CamerasFile => Path.Combine(ConfigDirectory, "cameras.json");

    /// <summary>The game_master configuration, alongside the launcher's settings under struckout.</summary>
    public string GameMasterConfigFile => Path.Combine(Path.GetDirectoryName(ConfigDirectory)!, "game_master", "config.toml");

    public string InstallStateFile => Path.Combine(DataDirectory, "install-state.json");

    public string ModulesDirectory => Path.Combine(DataDirectory, "modules");

    public string ToolsDirectory => Path.Combine(DataDirectory, "tools");

    public string LogsDirectory => Path.Combine(DataDirectory, "logs");

    /// <summary>The warning sound of <see cref="AlarmSound"/>, written by the launcher itself.</summary>
    public string AlarmSoundFile => Path.Combine(DataDirectory, "camera-alarm.wav");

    public string ModuleDirectory(string slug) => Path.Combine(ModulesDirectory, slug);

    public static AppPaths ForCurrentUser(HostPlatform platform)
    {
        string config;
        string data;
        switch (platform.Os)
        {
            case OperatingSystemKind.Windows:
                config = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
                data = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
                break;
            case OperatingSystemKind.MacOS:
                config = data = Path.Combine(
                    Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), "Library", "Application Support");
                break;
            case OperatingSystemKind.Linux:
                // .NET resolves these from $XDG_CONFIG_HOME and $XDG_DATA_HOME like the dirs crate does.
                config = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
                data = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
                break;
            default:
                throw new InvalidOperationException($"unknown OS {platform.Os}");
        }

        if (config.Length == 0 || data.Length == 0)
        {
            throw new InvalidOperationException("ホームディレクトリが見つかりません");
        }

        return new AppPaths(Path.Combine(config, "struckout", "launcher"), Path.Combine(data, "struckout", "launcher"));
    }
}
