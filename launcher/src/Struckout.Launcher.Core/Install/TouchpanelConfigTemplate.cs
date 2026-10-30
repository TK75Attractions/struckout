namespace Struckout.Launcher.Core.Install;

/// <summary>Creates touchpanel's own config when it is first installed, preserving edits on updates.</summary>
internal static class TouchpanelConfigTemplate
{
    public const string Content = """
        # touchpanel の設定。game-master が別の PC にある場合は address を変更する。
        address = "127.0.0.1"
        machine-id = 1
        disable-multiplay = false

        """;

    /// <returns>Whether the file was written.</returns>
    public static bool WriteIfMissing(string path)
    {
        if (File.Exists(path))
        {
            return false;
        }

        Directory.CreateDirectory(Path.GetDirectoryName(Path.GetFullPath(path))!);
        File.WriteAllText(path, Content);
        return true;
    }
}
