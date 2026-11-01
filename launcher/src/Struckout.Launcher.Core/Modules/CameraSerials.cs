namespace Struckout.Launcher.Core.Modules;

/// <summary>A camera of <c>cameras.json</c> (by its position) and the phone that should be it.</summary>
public sealed record SerialAssignment(int Index, string Name, string Serial);

/// <summary>How the connected phones match the cameras of <c>cameras.json</c>.</summary>
/// <param name="Automatic">The serials that can be written without asking: one new phone for one missing camera.</param>
/// <param name="UnknownDevices">Connected phones that no camera has.</param>
/// <param name="MissingCameras">The positions of the cameras whose phone isn't connected.</param>
public sealed record SerialPlan(
    IReadOnlyList<SerialAssignment> Automatic, IReadOnlyList<string> UnknownDevices, IReadOnlyList<int> MissingCameras)
{
    /// <summary>New phones are connected, but which camera each one is can't be told.</summary>
    public bool NeedsChoice => Automatic.Count == 0 && UnknownDevices.Count > 0 && MissingCameras.Count > 0;
}

/// <summary>
/// Writes the serials of replaced phones to <c>cameras.json</c>, which used to be edited by hand (#391).
/// </summary>
/// <remarks>
/// A camera of <c>cameras.json</c> is a position on the booth (its pose and area), and nothing that adb reports
/// tells where a phone is mounted: the phones of a booth are usually the same model. So a serial is written only
/// when there's one way to do it, a single new phone and a single camera whose phone is missing, which is what
/// replacing a phone looks like. Otherwise the user chooses (<see cref="CamerasFile.AssignSerial"/>).
/// A camera whose phone is connected is never changed.
/// </remarks>
public static class CameraSerials
{
    public static SerialPlan Plan(IReadOnlyList<CameraEntry> cameras, IReadOnlyList<string> connected)
    {
        var known = cameras.Select(c => c.Serial).ToHashSet(StringComparer.Ordinal);
        var unknown = connected.Where(serial => !known.Contains(serial)).Distinct(StringComparer.Ordinal).ToList();
        var missing = Enumerable.Range(0, cameras.Count).Where(i => !connected.Contains(cameras[i].Serial)).ToList();
        IReadOnlyList<SerialAssignment> automatic = unknown.Count == 1 && missing.Count == 1
            ? [new SerialAssignment(missing[0], cameras[missing[0]].Name, unknown[0])]
            : [];
        return new SerialPlan(automatic, unknown, missing);
    }

    /// <summary>Writes the serials that need no choice and reports the phones that need one.</summary>
    /// <returns>Whether <paramref name="file"/> changed.</returns>
    public static bool AssignAutomatically(CamerasFile file, IReadOnlyList<string> connected, IProgress<string> log)
    {
        var plan = Plan(file.Cameras, connected);
        foreach (var assignment in plan.Automatic)
        {
            var previous = file.Cameras[assignment.Index].Serial;
            file.AssignSerial(assignment.Index, assignment.Serial);
            log.Report($"camera: {assignment.Name} のスマホが {previous} から {assignment.Serial} に替わったので、{file.FilePath} に書きました");
        }

        if (plan.NeedsChoice)
        {
            log.Report(
                $"camera: {file.FilePath} にないスマホ ({string.Join("、", plan.UnknownDevices)}) がつながっています。"
                + $"どれが {string.Join("、", plan.MissingCameras.Select(i => file.Cameras[i].Name))} か決められないので、"
                + "起動の画面の「カメラの割り当て」で選んでください");
        }

        return plan.Automatic.Count > 0;
    }
}
