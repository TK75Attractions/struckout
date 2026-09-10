using System.Collections.Generic;
using NUnit.Framework;
using Struckout.Infrastructure;
using Tk75Attractions.Struckout.V1;

namespace Struckout.Tests
{
    /// <summary>
    /// Fake モードでゲームが始まることを固定する。
    ///
    /// 呼ぶ側は ConnectAsync を await したあとで購読するのに、ダミーは
    /// ConnectAsync の中で開始を発火する。この順序で開始が落ちると
    /// 的が一つも出ず、対向なしでの描画調整が成立しなくなる。
    /// </summary>
    public class FakeGameMasterClientTests
    {
        [Test]
        public void 接続してから購読しても開始を受け取れる()
        {
            var client = new FakeGameMasterClient();
            var received = new List<Difficulty>();

            // 実際の起動順。NetworkBootstrap が接続し、そのあと GameBootstrap が購読する。
            client.ConnectAsync();
            client.GameStarted += received.Add;

            Assert.That(received, Has.Count.EqualTo(1));
            Assert.That(received[0], Is.EqualTo(Difficulty.Normal));
        }

        [Test]
        public void 購読してから接続しても開始は一度だけ()
        {
            var client = new FakeGameMasterClient();
            var received = new List<Difficulty>();

            client.GameStarted += received.Add;
            client.ConnectAsync();

            Assert.That(received, Has.Count.EqualTo(1));
        }

        [Test]
        public void 二回接続しても開始は増えない()
        {
            var client = new FakeGameMasterClient();
            var received = new List<Difficulty>();

            client.GameStarted += received.Add;
            client.ConnectAsync();
            client.ConnectAsync();

            Assert.That(received, Has.Count.EqualTo(1));
        }

        [Test]
        public void 切断したあとに購読しても開始は流れない()
        {
            var client = new FakeGameMasterClient();
            var received = new List<Difficulty>();

            client.ConnectAsync();
            client.DisconnectAsync();
            client.GameStarted += received.Add;

            Assert.That(received, Is.Empty);
        }

        [Test]
        public void 購読を外すと届かなくなる()
        {
            var client = new FakeGameMasterClient();
            var received = new List<Difficulty>();
            void Handler(Difficulty d) => received.Add(d);

            client.GameStarted += Handler;
            client.GameStarted -= Handler;
            client.ConnectAsync();

            Assert.That(received, Is.Empty);
        }
    }
}
