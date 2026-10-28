using Microsoft.Extensions.Logging.Abstractions;
using System;
using System.Collections.Generic;
using NUnit.Framework;
using Struckout.Domain;
using Struckout.Infrastructure;

namespace Struckout.Tests.Tests.EditMode
{
    /// <summary>
    /// 接続先の解決。優先順位は コマンドライン &gt; 環境変数 &gt; Inspector。
    /// </summary>
    public class NetworkSettingsResolverTests
    {
        private static Dictionary<string, string> Args(params string[] pairs)
        {
            var result = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            for (var i = 0; i + 1 < pairs.Length; i += 2) result[pairs[i]] = pairs[i + 1];
            return result;
        }

        private static Func<string, string> Env(params string[] pairs)
        {
            var map = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            for (var i = 0; i + 1 < pairs.Length; i += 2) map[pairs[i]] = pairs[i + 1];
            return name => map.TryGetValue(name, out var value) ? value : null;
        }

        private static NetworkSettings Resolve(
            NetworkSettings inspector = null,
            Dictionary<string, string> args = null,
            Func<string, string> env = null)
            => new NetworkSettingsResolver(NullLogger<NetworkSettingsResolver>.Instance).Resolve(inspector, args, env);

        [Test]
        public void 上書きが無ければ_Inspector_の値が使われる()
        {
            var inspector = new NetworkSettings { trackerHost = "10.0.0.1", trackerPort = 6000 };

            var result = Resolve(inspector);

            Assert.That(result.trackerHost, Is.EqualTo("10.0.0.1"));
            Assert.That(result.trackerPort, Is.EqualTo(6000));
        }

        [Test]
        public void 環境変数は_Inspector_より優先される()
        {
            var inspector = new NetworkSettings { trackerHost = "10.0.0.1" };

            var result = Resolve(inspector, env: Env("STRUCKOUT_TRACKER_HOST", "192.168.0.10"));

            Assert.That(result.trackerHost, Is.EqualTo("192.168.0.10"));
        }

        [Test]
        public void コマンドラインは環境変数より優先される()
        {
            var inspector = new NetworkSettings { trackerHost = "10.0.0.1" };

            var result = Resolve(
                inspector,
                args: Args("trackerHost", "172.16.0.5"),
                env: Env("STRUCKOUT_TRACKER_HOST", "192.168.0.10"));

            Assert.That(result.trackerHost, Is.EqualTo("172.16.0.5"));
        }

        [Test]
        public void ネットワークモードを大文字小文字を問わず解釈する()
        {
            Assert.That(Resolve(args: Args("networkMode", "fake")).mode, Is.EqualTo(NetworkMode.Fake));
            Assert.That(Resolve(args: Args("networkMode", "REAL")).mode, Is.EqualTo(NetworkMode.Real));
        }

        [Test]
        public void 解釈できない値は_Inspector_の値に戻す()
        {
            var inspector = new NetworkSettings { mode = NetworkMode.Fake, trackerPort = 5000 };

            var result = Resolve(
                inspector,
                args: Args("networkMode", "somethingElse", "trackerPort", "notANumber"));

            Assert.That(result.mode, Is.EqualTo(NetworkMode.Fake));
            Assert.That(result.trackerPort, Is.EqualTo(5000));
        }

        [Test]
        public void Inspector_が空でも既定値で補われる()
        {
            var inspector = new NetworkSettings { trackerHost = "", masterHost = null, trackerPort = 0 };

            var result = Resolve(inspector);

            Assert.That(result.trackerHost, Is.EqualTo("127.0.0.1"));
            Assert.That(result.masterHost, Is.EqualTo("127.0.0.1"));
            Assert.That(result.trackerPort, Is.EqualTo(5000));
        }

        [Test]
        public void 試行回数は1未満にならない()
        {
            var result = Resolve(args: Args("connectAttempts", "0"));

            Assert.That(result.connectAttempts, Is.EqualTo(1));
        }

        [Test]
        public void 元の設定を書き換えない()
        {
            var inspector = new NetworkSettings { trackerHost = "10.0.0.1" };

            Resolve(inspector, args: Args("trackerHost", "172.16.0.5"));

            Assert.That(inspector.trackerHost, Is.EqualTo("10.0.0.1"),
                "Inspector の値をそのまま書き換えると、シーンの設定が実行時に汚れる");
        }
    }
}
