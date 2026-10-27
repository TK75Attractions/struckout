using System;
using System.Threading.Tasks;
using System.Collections.Generic;
using Struckout.Domain.TargetDefinition;
using NUnit.Framework;
using Struckout.Domain;
using Struckout.Infrastructure.Fake;
using Tk75Attractions.Struckout.GameMaster.V1;

namespace Struckout.Tests.Tests.EditMode
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
            client.GameStarted += (difficulty, _) => received.Add(difficulty);

            Assert.That(received, Has.Count.EqualTo(1));
            Assert.That(received[0], Is.EqualTo(Difficulty.Normal));
        }

        [Test]
        public void 購読してから接続しても開始は一度だけ()
        {
            var client = new FakeGameMasterClient();
            var received = new List<Difficulty>();

            client.GameStarted += (difficulty, _) => received.Add(difficulty);
            client.ConnectAsync();

            Assert.That(received, Has.Count.EqualTo(1));
        }

        [Test]
        public void 二回接続しても開始は増えない()
        {
            var client = new FakeGameMasterClient();
            var received = new List<Difficulty>();

            client.GameStarted += (difficulty, _) => received.Add(difficulty);
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
            client.GameStarted += (difficulty, _) => received.Add(difficulty);

            Assert.That(received, Is.Empty);
        }

        [Test]
        public void 購読を外すと届かなくなる()
        {
            var client = new FakeGameMasterClient();
            var received = new List<Difficulty>();
            void Handler(Difficulty d, PlayerId _) => received.Add(d);

            client.GameStarted += Handler;
            client.GameStarted -= Handler;
            client.ConnectAsync();

            Assert.That(received, Is.Empty);
        }
        [Test]
        public async Task Registered_targets_score_repeatedly_until_removed()
        {
            var client = new FakeGameMasterClient();
            var scores = new List<uint>();
            client.ScoreChanged += scores.Add;
            var added = new List<Guid>();
            var removed = new List<Guid>();
            client.TargetAdded += target => added.Add(target.Id);
            client.TargetRemoved += removed.Add;
            try
            {
                Assert.That(await client.AddTargetAsync(default, 200), Is.False);
                await client.ConnectAsync();
                Assert.That(await client.AddTargetAsync(new TargetCoordinate(100, 200), 200), Is.True);
                Assert.That(added, Has.Count.EqualTo(1));
                var id = added[0];
                Assert.That(await client.HitTargetAsync(id), Is.True);
                Assert.That(await client.HitTargetAsync(id), Is.True);
                Assert.That(scores, Is.EqualTo(new uint[] { 0, 500, 1000 }));
                Assert.That(await client.RemoveTargetAsync(id), Is.True);
                Assert.That(removed, Is.EqualTo(new[] { id }));
                Assert.That(await client.HitTargetAsync(id), Is.False);
                Assert.That(await client.RemoveTargetAsync(id), Is.False);
                Assert.That(await client.HitTargetAsync(Guid.NewGuid()), Is.False);
                await client.DisconnectAsync();
                Assert.That(await client.HitTargetAsync(id), Is.False);
                await client.ConnectAsync();
                Assert.That(await client.HitTargetAsync(id), Is.False);
                Assert.That(scores[scores.Count - 1], Is.Zero);
            }
            finally { await client.DisconnectAsync(); }
        }

        [TestCase(0)]
        [TestCase(-1)]
        [TestCase(double.NaN)]
        [TestCase(double.PositiveInfinity)]
        [TestCase(1e-30)]
        public async Task Invalid_scores_do_not_emit_events(double radius)
        {
            var client = new FakeGameMasterClient();
            try
            {
                await client.ConnectAsync();
                var scores = new List<uint>();
                client.ScoreChanged += scores.Add;
                var id = Guid.Empty;
                client.TargetAdded += target => id = target.Id;
                Assert.That(await client.AddTargetAsync(default, radius), Is.True);
                Assert.That(await client.HitTargetAsync(id), Is.False);
                Assert.That(scores, Is.Empty);
            }
            finally { await client.DisconnectAsync(); }
        }

        [Test]
        public async Task Disconnect_does_not_emit_removals_and_reconnect_cleans_up_by_events_before_start()
        {
            var client = new FakeGameMasterClient();
            var events = new List<string>();
            client.TargetAdded += _ => events.Add("added");
            client.TargetRemoved += _ => events.Add("removed");
            client.GameStarted += (_, __) => events.Add("started");
            try
            {
                await client.ConnectAsync();
                await client.AddTargetAsync(default, 200);
                events.Clear();
                await client.DisconnectAsync();
                Assert.That(events, Is.Empty);
                await client.ConnectAsync();
                Assert.That(events, Is.EqualTo(new[] { "removed", "started" }));
            }
            finally { await client.DisconnectAsync(); }
        }

        [Test]
        public async Task Game_finished_does_not_emit_target_removals()
        {
            var client = new FakeGameMasterClient(TimeSpan.FromMilliseconds(250), TimeSpan.Zero);
            var finished = new TaskCompletionSource<bool>(TaskCreationOptions.RunContinuationsAsynchronously);
            var removed = new List<Guid>();
            client.GameFinished += _ => finished.TrySetResult(true);
            client.TargetRemoved += removed.Add;
            try
            {
                await client.ConnectAsync();
                Assert.That(await client.AddTargetAsync(default, 200), Is.True);
                Assert.That(await Task.WhenAny(finished.Task, Task.Delay(TimeSpan.FromSeconds(5))),
                    Is.SameAs(finished.Task));
                Assert.That(removed, Is.Empty);
            }
            finally { await client.DisconnectAsync(); }
        }

        [Test]
        public async Task Fake_publishes_the_full_screen_flow_and_retains_finished_results()
        {
            var client = new FakeGameMasterClient(TimeSpan.Zero, TimeSpan.Zero);
            var events = new List<string>();
            var playerId = default(PlayerId);
            var gameId = default(GameId);
            client.GameStarted += (_, player) => { playerId = player; events.Add("started"); };
            client.GameFinished += game => { gameId = game; events.Add("finished"); };
            client.RankingScreenRequested += () => events.Add("ranking");
            client.HomeScreenRequested += () => events.Add("home");
            try
            {
                await client.ConnectAsync();
                Assert.That(events, Is.EqualTo(new[] { "started", "finished", "ranking", "home" }));
                Assert.That(gameId.Value, Is.GreaterThan(0));
                Assert.That(await client.GetGameResultAsync(gameId), Is.EqualTo(0));
                var ranking = await client.GetRankingAsync(10);
                var history = await client.GetGamesByPlayerAsync(playerId);
                Assert.That(ranking.Count, Is.EqualTo(10));
                Assert.That(history, Has.Some.Matches<PlayerGame>(game => game.GameId.Equals(gameId)));
                Assert.That(await client.GetGameResultAsync(new GameId(9999)), Is.Null);
            }
            finally { await client.DisconnectAsync(); }
        }

    }
}
