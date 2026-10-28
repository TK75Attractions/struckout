using System;
using System.Text.RegularExpressions;
using Microsoft.Extensions.Logging;
using NUnit.Framework;
using Struckout.Application;
using Struckout.Infrastructure;
using Struckout.Infrastructure.Fake;
using Struckout.Infrastructure.Grpc;
using Struckout.Infrastructure.TCP;
using Struckout.Domain;
using Struckout.Unity;
using Tk75Attractions.Struckout.V1;
using UnityEngine.TestTools;
using VContainer;
using LogType = UnityEngine.LogType;

namespace Struckout.Tests.Tests.EditMode
{
    public class UnityLoggerTests
    {
        [TestCase(LogLevel.Trace, LogType.Log)]
        [TestCase(LogLevel.Debug, LogType.Log)]
        [TestCase(LogLevel.Information, LogType.Log)]
        [TestCase(LogLevel.Warning, LogType.Warning)]
        [TestCase(LogLevel.Error, LogType.Error)]
        [TestCase(LogLevel.Critical, LogType.Error)]
        public void Writes_category_and_formatted_values_at_the_Unity_log_level(LogLevel level, LogType expected)
        {
            var logger = new UnityLogger<UnityLoggerTests>();
            LogAssert.Expect(expected, $"[{typeof(UnityLoggerTests).FullName}] Target 42");

            logger.Log(level, "Target {TargetId}", 42);
        }

        [Test]
        public void Includes_exception_details_with_the_message()
        {
            var logger = new UnityLogger<UnityLoggerTests>();
            var exception = new InvalidOperationException("failure", new ArgumentException("cause"));
            LogAssert.Expect(LogType.Error,
                $"[{typeof(UnityLoggerTests).FullName}] Request failed\n{exception}");

            logger.LogError(exception, "Request failed");
        }

        [Test]
        public void None_does_not_format_or_write_a_message()
        {
            var logger = new UnityLogger<UnityLoggerTests>();
            Assert.That(logger.IsEnabled(LogLevel.None), Is.False);

            logger.Log(LogLevel.None, default, "ignored", null,
                (state, exception) => throw new InvalidOperationException("Must not format a disabled log"));
            LogAssert.NoUnexpectedReceived();
        }

        [Test]
        public void Container_resolves_category_loggers_and_both_game_master_clients()
        {
            var builder = new ContainerBuilder();
            builder.Register(typeof(UnityLogger<>), Lifetime.Singleton).As(typeof(ILogger<>));
            builder.RegisterInstance(new NetworkSettings());
            builder.Register<Struckout.Application.Interface.IGameMasterClient>(container => new FakeGameMasterClient(
                container.Resolve<ILogger<FakeGameMasterClient>>()), Lifetime.Singleton);
            builder.Register<GrpcGameMasterClient>(Lifetime.Singleton);
            using var container = builder.Build();

            Assert.That(container.Resolve<ILogger<GameRuntime>>(), Is.TypeOf<UnityLogger<GameRuntime>>());
            Assert.That(container.Resolve<ILogger<TcpClientBase<ProjectorPacket>>>(),
                Is.TypeOf<UnityLogger<TcpClientBase<ProjectorPacket>>>());
            Assert.That(container.Resolve<Struckout.Application.Interface.IGameMasterClient>(), Is.TypeOf<FakeGameMasterClient>());
            Assert.That(container.Resolve<GrpcGameMasterClient>(), Is.Not.Null);
        }

        [Test]
        public void Invalid_network_settings_use_the_supplied_logger()
        {
            var resolver = new NetworkSettingsResolver(new UnityLogger<NetworkSettingsResolver>());
            LogAssert.Expect(LogType.Warning,
                new Regex("\\[Struckout.Infrastructure.NetworkSettingsResolver\\].*invalid.*Real"));

            var settings = resolver.Resolve(new NetworkSettings(),
                new System.Collections.Generic.Dictionary<string, string> { ["networkMode"] = "invalid" },
                _ => null);

            Assert.That(settings.mode, Is.EqualTo(NetworkMode.Real));
        }
    }
}
