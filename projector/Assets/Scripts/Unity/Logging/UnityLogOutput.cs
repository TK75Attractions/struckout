using System;
using Microsoft.Extensions.Logging;
using Debug = UnityEngine.Debug;

namespace Struckout.Unity.Logging
{
    /// <summary>
    /// Writes local diagnostics without routing them back through Sentry's log capture.
    /// Exception objects are forwarded to Sentry separately by ProjectorLogger.
    /// </summary>
    internal static class UnityLogOutput
    {
        public static void Write(LogLevel level, string category, string message, Exception exception)
        {
            var output = $"[{category}] {message}";
            if (exception != null) output += $"\n{exception}";
            switch (level)
            {
                case LogLevel.Critical:
                case LogLevel.Error:
                    Debug.LogError(output);
                    break;
                case LogLevel.Warning:
                    Debug.LogWarning(output);
                    break;
                default:
                    Debug.Log(output);
                    break;
            }
        }
    }
}
