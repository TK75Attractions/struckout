using System;
using System.Collections.Generic;
using System.Globalization;
using Microsoft.Extensions.Logging;
using Sentry;
using Struckout.Domain;

namespace Struckout.Unity.Logging
{
    /// <summary>
    /// Keeps structured properties and scopes when forwarding logs to Sentry,
    /// independently of the Unity Console output and the SDK's lifetime.
    /// </summary>
    public sealed class ProjectorLogger<T> : ILogger<T>
    {
        private static readonly string Category = typeof(T).FullName;
        private readonly IHub _hub;
        private readonly IExternalScopeProvider _scopes;
        private readonly NetworkSettings _settings;

        /// <summary>
        /// Creates a logger without machine identity, only for the composition root
        /// before <see cref="NetworkSettings"/> has been resolved.
        /// </summary>
        public ProjectorLogger(IHub hub, IExternalScopeProvider scopes)
        {
            _hub = hub ?? throw new ArgumentNullException(nameof(hub));
            _scopes = scopes ?? throw new ArgumentNullException(nameof(scopes));
        }

        /// <summary>
        /// Creates a logger that tags every output with the machine identity.
        /// VContainer ignores default parameter values and uses the constructor with the most parameters,
        /// so container-resolved loggers always come here and need <see cref="NetworkSettings"/> registered.
        /// </summary>
        public ProjectorLogger(IHub hub, IExternalScopeProvider scopes, NetworkSettings settings)
            : this(hub, scopes)
        {
            _settings = settings ?? throw new ArgumentNullException(nameof(settings));
        }

        public IDisposable BeginScope<TState>(TState state) => _scopes.Push(state);

        public bool IsEnabled(LogLevel logLevel) => logLevel is >= LogLevel.Trace and <= LogLevel.Critical;

        public void Log<TState>(LogLevel logLevel, EventId eventId, TState state,
            Exception exception, Func<TState, Exception, string> formatter)
        {
            if (!IsEnabled(logLevel)) return;
            if (formatter == null) throw new ArgumentNullException(nameof(formatter));

            var message = formatter(state, exception);
            UnityLogOutput.Write(logLevel, Category, message, exception, _settings?.machineId);
            if (!_hub.IsEnabled) return;

            var attributes = new Dictionary<string, object>(StringComparer.Ordinal);
            _scopes.ForEachScope((scope, values) => AddProperties(scope, values, true), attributes);
            AddProperties(state, attributes);
            var template = state is IEnumerable<KeyValuePair<string, object>> properties
                ? OriginalFormat(properties) : null;
            if (_settings != null) attributes["machine_id"] = _settings.machineId;
            attributes["logger.category"] = Category;
            attributes["event.id"] = eventId.Id;
            if (eventId.Name != null) attributes["event.name"] = eventId.Name;
            if (template != null) attributes["sentry.message.template"] = template;
            if (exception != null)
            {
                attributes["exception.type"] = exception.GetType().FullName;
                attributes["exception.message"] = exception.Message;
                attributes["exception.stacktrace"] = exception.ToString();
            }

            void ConfigureLog(SentryLog log)
            {
                foreach (var attribute in attributes) log.SetAttribute(attribute.Key, attribute.Value);
            }

            // ILogger uses named placeholders; Sentry's logger expects string.Format placeholders.
            // Pass the formatted message without parameters and preserve the template as an attribute.
            switch (logLevel)
            {
                case LogLevel.Trace: _hub.Logger.LogTrace(ConfigureLog, message); break;
                case LogLevel.Debug: _hub.Logger.LogDebug(ConfigureLog, message); break;
                case LogLevel.Information: _hub.Logger.LogInfo(ConfigureLog, message); break;
                case LogLevel.Warning: _hub.Logger.LogWarning(ConfigureLog, message); break;
                case LogLevel.Error: _hub.Logger.LogError(ConfigureLog, message); break;
                case LogLevel.Critical: _hub.Logger.LogFatal(ConfigureLog, message); break;
            }

            if (logLevel < LogLevel.Error) return;

            var sentryEvent = exception == null ? new SentryEvent() : new SentryEvent(exception);
            sentryEvent.Message = new SentryMessage { Formatted = message, Message = template };
            sentryEvent.Logger = Category;
            sentryEvent.Level = logLevel == LogLevel.Critical ? SentryLevel.Fatal : SentryLevel.Error;
            foreach (var attribute in attributes) sentryEvent.SetExtra(attribute.Key, attribute.Value);
            if (_settings != null) sentryEvent.SetTag("machine_id", _settings.machineId.ToString(CultureInfo.InvariantCulture));
            _hub.CaptureEvent(sentryEvent);
        }

        private static string OriginalFormat(IEnumerable<KeyValuePair<string, object>> properties)
        {
            foreach (var property in properties)
                if (property.Key == "{OriginalFormat}") return property.Value as string;
            return null;
        }

        private static void AddProperties(object state, Dictionary<string, object> attributes, bool scope = false)
        {
            if (state is not IEnumerable<KeyValuePair<string, object>> properties)
            {
                if (scope && state != null) attributes["scope"] = state.ToString();
                return;
            }

            foreach (var property in properties)
            {
                if (property.Key == "{OriginalFormat}") continue;
                if (property.Value == null) attributes.Remove(property.Key);
                else attributes[property.Key] = Normalize(property.Value);
            }
        }

        private static object Normalize(object value) => value switch
        {
            string or bool or sbyte or byte or short or ushort or int or uint or long or double => value,
            float number => (double)number,
            decimal number => (double)number,
            ulong number when number <= long.MaxValue => (long)number,
            IFormattable formattable => formattable.ToString(null, CultureInfo.InvariantCulture),
            _ => value.ToString(),
        };
    }
}
