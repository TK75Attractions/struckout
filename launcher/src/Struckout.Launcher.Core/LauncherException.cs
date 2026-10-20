namespace Struckout.Launcher.Core;

/// <summary>
/// A failure the user can act on (network, a missing tool, a broken setting, ...).
/// The message is in Japanese and is shown to the user as is.
/// </summary>
public class LauncherException(string message, Exception? innerException = null) : Exception(message, innerException);
