using System;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Logging.Abstractions;
using Debug = UnityEngine.Debug;

namespace Struckout.Unity
{
    /// <summary>
    /// Sends category-based .NET logs to the Unity Console and Player.log so the
    /// other projects can log without depending on Unity APIs.
    /// </summary>
    public sealed class UnityLogger<T> : ILogger<T>
    {
        private static readonly string Category = typeof(T).FullName;

        // There are no ambient Unity log scopes; keep the standard no-op scope behavior.
        public IDisposable BeginScope<TState>(TState state) => NullLogger.Instance.BeginScope(state);

        public bool IsEnabled(LogLevel logLevel) => logLevel != LogLevel.None;

        public void Log<TState>(LogLevel logLevel, EventId eventId, TState state,
            Exception exception, Func<TState, Exception, string> formatter)
        {
            if (!IsEnabled(logLevel)) return;

            var message = $"[{Category}] {formatter(state, exception)}";
            if (exception != null) message += $"\n{exception}";

            switch (logLevel)
            {
                case LogLevel.Critical:
                case LogLevel.Error:
                    Debug.LogError(message);
                    break;
                case LogLevel.Warning:
                    Debug.LogWarning(message);
                    break;
                default:
                    Debug.Log(message);
                    break;
            }
        }
    }
}
