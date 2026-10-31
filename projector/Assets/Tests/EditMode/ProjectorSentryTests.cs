using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text;
using System.Text.RegularExpressions;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.Extensions.Logging;
using NUnit.Framework;
using Sentry;
using Sentry.Extensibility;
using Sentry.Protocol.Envelopes;
using Sentry.Unity;
using Struckout.Infrastructure;
using Struckout.Unity;
using Struckout.Unity.Logging;
using UnityEngine.TestTools;
using VContainer;
using LogType = UnityEngine.LogType;
using Debug = UnityEngine.Debug;
using SentrySdk = Sentry.Unity.SentrySdk;

namespace Struckout.Tests.Tests.EditMode
{
    // The Unity test runner is sequential. Each test resets the public SDK and uses only an in-memory transport.
    public class ProjectorSentryTests
    {
        private sealed class MemoryTransport : ITransport
        {
            public readonly ConcurrentQueue<string> Envelopes = new();

            public async Task SendEnvelopeAsync(Envelope envelope, CancellationToken cancellationToken = default)
            {
                using var stream = new MemoryStream();
                await envelope.SerializeAsync(stream, null, cancellationToken);
                Envelopes.Enqueue(Encoding.UTF8.GetString(stream.ToArray()));
            }
        }

        private readonly ConcurrentQueue<SentryLog> _logs = new();
        private readonly ConcurrentQueue<SentryEvent> _events = new();
        private MemoryTransport _transport;
        private LoggerExternalScopeProvider _scopes;

        [SetUp]
        public void SetUp()
        {
            SentrySdk.Close();
            _logs.Clear();
            _events.Clear();
            _transport = new MemoryTransport();
            _scopes = new LoggerExternalScopeProvider();
            var options = ScriptableSentryUnityOptions.LoadSentryUnityOptions();
            // This dummy DSN only satisfies SDK validation. All envelopes stay in memory.
            options.Dsn = "https://public@example.invalid/1";
            options.Transport = _transport;
            options.CacheDirectoryPath = null;
            options.AutoSessionTracking = false;
            options.AutoStartupTraces = false;
            options.AutoSceneLoadTraces = false;
            options.TracesSampleRate = 0;
            options.SetBeforeSendLog(log => { _logs.Enqueue(log); return log; });
            options.SetBeforeSend((evt, hint) => { _events.Enqueue(evt); return evt; });
            SentrySdk.Init(options);
        }

        [TearDown]
        public void TearDown() => SentrySdk.Close();

        private ProjectorLogger<T> Logger<T>() => new(HubAdapter.Instance, _scopes);

        private static object Attribute(SentryLog log, string key)
        {
            Assert.That(log.TryGetAttribute(key, out var value), Is.True, key);
            return value;
        }

        [TestCase(LogLevel.Trace, SentryLogLevel.Trace, LogType.Log, false)]
        [TestCase(LogLevel.Debug, SentryLogLevel.Debug, LogType.Log, false)]
        [TestCase(LogLevel.Information, SentryLogLevel.Info, LogType.Log, false)]
        [TestCase(LogLevel.Warning, SentryLogLevel.Warning, LogType.Warning, false)]
        [TestCase(LogLevel.Error, SentryLogLevel.Error, LogType.Error, true)]
        [TestCase(LogLevel.Critical, SentryLogLevel.Fatal, LogType.Error, true)]
        public async Task Sends_one_log_and_only_errors_create_events(
            LogLevel level, SentryLogLevel sentryLevel, LogType unityLevel, bool createsEvent)
        {
            var logger = Logger<ProjectorSentryTests>();
            var category = typeof(ProjectorSentryTests).FullName;
            LogAssert.Expect(unityLevel, $"[{category}] Target 42");

            logger.Log(level, new EventId(17, "TargetHit"), "Target {TargetId}", 42);
            await SentrySdk.FlushAsync(TimeSpan.FromSeconds(5));

            Assert.That(_logs.Count, Is.EqualTo(1));
            var log = _logs.Single();
            Assert.That(log.Level, Is.EqualTo(sentryLevel));
            Assert.That(log.Message, Is.EqualTo("Target 42"));
            Assert.That(Attribute(log, "TargetId"), Is.EqualTo(42));
            Assert.That(Attribute(log, "logger.category"), Is.EqualTo(category));
            Assert.That(Attribute(log, "event.id"), Is.EqualTo(17));
            Assert.That(Attribute(log, "event.name"), Is.EqualTo("TargetHit"));
            Assert.That(Attribute(log, "sentry.message.template"), Is.EqualTo("Target {TargetId}"));
            Assert.That(_events.Count, Is.EqualTo(createsEvent ? 1 : 0));
            if (createsEvent)
            {
                var evt = _events.Single();
                Assert.That(evt.Level, Is.EqualTo(level == LogLevel.Critical ? SentryLevel.Fatal : SentryLevel.Error));
                Assert.That(evt.Message.Formatted, Is.EqualTo("Target 42"));
                Assert.That(evt.Message.Message, Is.EqualTo("Target {TargetId}"));
                Assert.That(evt.Logger, Is.EqualTo(category));
                Assert.That(evt.Extra["TargetId"], Is.EqualTo(42));
            }
            Assert.That(string.Concat(_transport.Envelopes), Does.Contain("Target 42"));
        }

        [TestCase(LogLevel.Information, false)]
        [TestCase(LogLevel.Error, true)]
        [TestCase(LogLevel.Critical, true)]
        public async Task Preserves_exception_objects_and_details(LogLevel level, bool createsEvent)
        {
            var exception = new InvalidOperationException("failure", new ArgumentException("cause"));
            var category = typeof(ProjectorSentryTests).FullName;
            LogAssert.Expect(level >= LogLevel.Error ? LogType.Error : LogType.Log,
                $"[{category}] Request failed\n{exception}");

            Logger<ProjectorSentryTests>().Log(level, exception, "Request failed");
            await SentrySdk.FlushAsync(TimeSpan.FromSeconds(5));

            Assert.That(_logs.Count, Is.EqualTo(1));
            Assert.That(Attribute(_logs.Single(), "exception.type"), Is.EqualTo(exception.GetType().FullName));
            Assert.That(Attribute(_logs.Single(), "exception.stacktrace"), Does.Contain("cause"));
            Assert.That(_events.Count, Is.EqualTo(createsEvent ? 1 : 0));
            if (createsEvent) Assert.That(_events.Single().Exception, Is.SameAs(exception));
        }

        [Test]
        public async Task Scopes_are_shared_across_categories_and_restore_after_disposal()
        {
            var logger = Logger<ProjectorSentryTests>();
            var other = Logger<NetworkSettingsResolver>();
            LogAssert.Expect(LogType.Log, $"[{typeof(NetworkSettingsResolver).FullName}] Inner 3");
            LogAssert.Expect(LogType.Log, $"[{typeof(ProjectorSentryTests).FullName}] Outer");
            LogAssert.Expect(LogType.Log, $"[{typeof(ProjectorSentryTests).FullName}] Outside");

            using (logger.BeginScope(new Dictionary<string, object> { ["GameId"] = 1, ["Value"] = 1 }))
            {
                using (other.BeginScope(new Dictionary<string, object> { ["Value"] = 2 }))
                    other.LogInformation("Inner {Value}", 3);
                logger.LogInformation("Outer");
            }
            logger.LogInformation("Outside");
            await SentrySdk.FlushAsync(TimeSpan.FromSeconds(5));

            var logs = _logs.ToArray();
            Assert.That(logs, Has.Length.EqualTo(3));
            Assert.That(Attribute(logs[0], "GameId"), Is.EqualTo(1));
            Assert.That(Attribute(logs[0], "Value"), Is.EqualTo(3));
            Assert.That(Attribute(logs[1], "Value"), Is.EqualTo(1));
            Assert.That(logs[2].TryGetAttribute("GameId", out _), Is.False);
            Assert.That(logs[2].TryGetAttribute("Value", out _), Is.False);
        }

        [Test]
        public async Task Task_execution_contexts_do_not_share_scopes()
        {
            var logger = Logger<ProjectorSentryTests>();
            var ready = new TaskCompletionSource<bool>(TaskCreationOptions.RunContinuationsAsynchronously);
            var entered = 0;
            LogAssert.Expect(LogType.Log, new Regex(@"Task [12]$"));
            LogAssert.Expect(LogType.Log, new Regex(@"Task [12]$"));
            async Task Work(int id)
            {
                using (logger.BeginScope(new Dictionary<string, object> { ["GameId"] = id }))
                {
                    if (Interlocked.Increment(ref entered) == 2) ready.SetResult(true);
                    await ready.Task;
                    logger.LogInformation("Task {TaskId}", id);
                }
            }

            await Task.WhenAll(Task.Run(() => Work(1)), Task.Run(() => Work(2)));
            await SentrySdk.FlushAsync(TimeSpan.FromSeconds(5));

            Assert.That(_logs.Count, Is.EqualTo(2));
            foreach (var log in _logs) Assert.That(Attribute(log, "GameId"), Is.EqualTo(Attribute(log, "TaskId")));
        }

        [Test]
        public async Task Preserves_typed_values_and_literal_braces_without_reformatting()
        {
            var id = Guid.NewGuid();
            var logger = Logger<ProjectorSentryTests>();
            LogAssert.Expect(LogType.Log, new Regex("Value.*"));

            logger.LogInformation("Value {Enabled} {Count} {Distance} {Id} {{literal}}", true, 7, 1.5, id);
            await SentrySdk.FlushAsync(TimeSpan.FromSeconds(5));

            var log = _logs.Single();
            Assert.That(Attribute(log, "Enabled"), Is.TypeOf<bool>());
            Assert.That(Attribute(log, "Count"), Is.EqualTo(7));
            Assert.That(Attribute(log, "Distance"), Is.TypeOf<double>());
            Assert.That(Attribute(log, "Id"), Is.EqualTo(id.ToString()));
            Assert.That(log.Message, Does.EndWith("{literal}"));
        }

        [Test]
        public async Task Machine_identity_is_attached_to_local_logs_structured_logs_and_error_events()
        {
            var logger = new ProjectorLogger<ProjectorSentryTests>(HubAdapter.Instance, _scopes,
                new Struckout.Domain.NetworkSettings { machineId = 2 });
            LogAssert.Expect(LogType.Error, $"[{typeof(ProjectorSentryTests).FullName}] [machine_id=2] Identified");
            logger.LogError("Identified");
            await SentrySdk.FlushAsync(TimeSpan.FromSeconds(5));
            Assert.That(Attribute(_logs.Single(), "machine_id"), Is.EqualTo(2u));
            Assert.That(_events.Single().Tags["machine_id"], Is.EqualTo("2"));
        }

        [Test]
        public async Task Direct_Unity_logs_are_not_forwarded_but_Unity_exceptions_are()
        {
            LogAssert.Expect(LogType.Log, "direct info");
            LogAssert.Expect(LogType.Warning, "direct warning");
            LogAssert.Expect(LogType.Error, "direct error");
            Debug.Log("direct info");
            Debug.LogWarning("direct warning");
            Debug.LogError("direct error");
            await SentrySdk.FlushAsync(TimeSpan.FromSeconds(5));
            Assert.That(_logs, Is.Empty);
            Assert.That(_events, Is.Empty);

            LogAssert.Expect(LogType.Exception, new Regex("InvalidOperationException: direct exception"));
            Debug.LogException(new InvalidOperationException("direct exception"));
            await SentrySdk.FlushAsync(TimeSpan.FromSeconds(5));
            Assert.That(_logs, Is.Empty);
            Assert.That(_events.Count, Is.EqualTo(1));
        }

        [Test]
        public void Configured_Unity_log_capture_is_disabled()
        {
            var options = ScriptableSentryUnityOptions.LoadSentryUnityOptions();
            Assert.That(options.EnableLogs, Is.True);
            Assert.That(options.CaptureLogErrorEvents, Is.False);
            Assert.That(options.CaptureStructuredLogsForLogType.Values, Is.All.False);
            Assert.That(options.AddBreadcrumbsForLogType.Values, Is.All.False);
        }

        [Test]
        public void Container_disposal_does_not_close_the_existing_SDK()
        {
            var builder = new ContainerBuilder();
            builder.RegisterInstance<IHub>(HubAdapter.Instance);
            builder.RegisterInstance<IExternalScopeProvider>(_scopes);
            builder.RegisterInstance(new Struckout.Domain.NetworkSettings());
            builder.Register(typeof(ProjectorLogger<>), Lifetime.Singleton).As(typeof(ILogger<>));
            using (var container = builder.Build())
                Assert.That(container.Resolve<ILogger<ProjectorSentryTests>>(), Is.Not.Null);

            Assert.That(HubAdapter.Instance.IsEnabled, Is.True);
        }

        [Test]
        public void Disabled_SDK_keeps_local_output()
        {
            SentrySdk.Close();
            LogAssert.Expect(LogType.Error, $"[{typeof(ProjectorSentryTests).FullName}] Local error");

            Logger<ProjectorSentryTests>().LogError("Local error");

            Assert.That(_logs, Is.Empty);
            Assert.That(_events, Is.Empty);
        }
    }
}
