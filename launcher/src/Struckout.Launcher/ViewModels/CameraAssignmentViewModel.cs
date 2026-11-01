using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Struckout.Launcher.Core;
using Struckout.Launcher.Core.Install;
using Struckout.Launcher.Core.Modules;
using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.ViewModels;

/// <summary>A camera of <c>cameras.json</c> whose phone isn't connected.</summary>
/// <param name="Serial">The phone it had, which isn't connected.</param>
public sealed record CameraChoice(int Index, string Name, string Serial)
{
    public string Label => $"{Name} (前のスマホ: {Serial})";
}

/// <summary>A connected phone that no camera of <c>cameras.json</c> has.</summary>
public sealed partial class UnknownPhoneViewModel(string serial, string? model, IReadOnlyList<CameraChoice> cameras)
    : ObservableObject
{
    public string Serial { get; } = serial;

    public string Label { get; } = model is null ? serial : $"{serial} ({model})";

    public IReadOnlyList<CameraChoice> Cameras { get; } = cameras;

    [ObservableProperty]
    public partial CameraChoice? Camera { get; set; }
}

/// <summary>
/// Lets the user tell which camera a new phone is, when the launcher can't (<see cref="CameraSerials"/>).
/// </summary>
public sealed partial class CameraAssignmentViewModel(HostPlatform platform, AppPaths paths, ICommandRunner runner, LogViewModel log)
    : ObservableObject
{
    public ObservableCollection<UnknownPhoneViewModel> Phones { get; } = [];

    [ObservableProperty]
    public partial bool IsVisible { get; private set; }

    /// <summary>cameras.json must not change while ball_tracker and the phones use it.</summary>
    [ObservableProperty]
    [NotifyCanExecuteChangedFor(nameof(AssignCommand))]
    public partial bool IsLocked { get; set; }

    [ObservableProperty]
    public partial string Error { get; private set; } = "";

    public void Clear()
    {
        Phones.Clear();
        IsVisible = false;
        Error = "";
    }

    /// <summary>Looks for phones that need to be assigned. Nothing is shown when adb or cameras.json is missing.</summary>
    [RelayCommand]
    public async Task RefreshAsync()
    {
        if (new ToolLocator(platform, paths).Find(PrerequisiteId.Adb) is not { } executable)
        {
            Clear();
            return;
        }

        try
        {
            var file = CamerasFile.Load(paths.CamerasFile);
            var adb = new Adb(executable, runner);
            var connected = await adb.DevicesAsync(CancellationToken.None);
            var plan = CameraSerials.Plan(file.Cameras, connected);
            Phones.Clear();
            if (plan.NeedsChoice)
            {
                var models = await adb.ModelsAsync(CancellationToken.None);
                var choices = plan.MissingCameras
                    .Select(i => new CameraChoice(i, file.Cameras[i].Name, file.Cameras[i].Serial))
                    .ToList();
                foreach (var serial in plan.UnknownDevices)
                {
                    Phones.Add(new UnknownPhoneViewModel(serial, models.GetValueOrDefault(serial), choices));
                }
            }

            IsVisible = Phones.Count > 0;
            Error = "";
        }
        catch (LauncherException e)
        {
            Clear();
            log.Append($"camera: スマホの割り当てを確かめられません: {e.Message}");
        }
        catch (CommandNotFoundException e)
        {
            Clear();
            log.Append($"camera: スマホの割り当てを確かめられません: {e.Message}");
        }
    }

    private bool CanAssign() => !IsLocked;

    [RelayCommand(CanExecute = nameof(CanAssign))]
    private async Task AssignAsync()
    {
        var chosen = Phones.Where(phone => phone.Camera is not null).ToList();
        if (chosen.Count == 0)
        {
            Error = "どのカメラにするかを選んでください";
            return;
        }

        if (chosen.GroupBy(phone => phone.Camera!.Index).FirstOrDefault(group => group.Count() > 1) is { } duplicate)
        {
            Error = $"{duplicate.First().Camera!.Name} に 2 台を選んでいます";
            return;
        }

        try
        {
            var file = CamerasFile.Load(paths.CamerasFile);
            foreach (var phone in chosen)
            {
                var camera = phone.Camera!;
                if (file.Cameras.ElementAtOrDefault(camera.Index)?.Serial != camera.Serial)
                {
                    throw new LauncherException($"{file.FilePath} が変わりました。「探し直す」を押してから選び直してください");
                }

                file.AssignSerial(camera.Index, phone.Serial);
                log.Append($"camera: {camera.Name} のスマホを {camera.Serial} から {phone.Serial} にして、{file.FilePath} に書きました");
            }
        }
        catch (LauncherException e)
        {
            Error = e.Message;
            return;
        }

        await RefreshAsync();
    }
}
