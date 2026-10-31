using Microsoft.Extensions.Logging.Abstractions;
using System;
using System.Threading.Tasks;
using System.Collections.Generic;
using Struckout.Domain.TargetDefinition;
using NUnit.Framework;
using Struckout.Domain;
using Struckout.Domain.Ranking;
using Struckout.Infrastructure.Fake;
using Struckout.Application.Interface;
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
        public async Task 接続してから購読しても開始を受け取れる()
        {
            var client = new FakeGameMasterClient(NullLogger<FakeGameMasterClient>.Instance);
            var received = new List<Difficulty>();

            // 実際の起動順。NetworkBootstrap が接続し、そのあと GameBootstrap が購読する。
            try
            {
                await client.ConnectAsync();
                client.GameStarted += (difficulty, mode, _, duration) => received.Add(difficulty);

                Assert.That(received, Has.Count.EqualTo(1));
                Assert.That(received[0], Is.EqualTo(Difficulty.Normal));
            }
            finally { await client.DisconnectAsync(); }
        }

        [Test]
        public async Task 購読してから接続しても開始は一度だけ()
        {
            var client = new FakeGameMasterClient(NullLogger<FakeGameMasterClient>.Instance);
            var received = new List<Difficulty>();

            try
            {
                client.GameStarted += (difficulty, mode, _, duration) => received.Add(difficulty);
                await client.ConnectAsync();

                Assert.That(received, Has.Count.EqualTo(1));
            }
            finally { await client.DisconnectAsync(); }
        }

        [Test]
        public async Task 二回接続しても開始は増えない()
        {
            var client = new FakeGameMasterClient(NullLogger<FakeGameMasterClient>.Instance);
            var received = new List<Difficulty>();

            try
            {
                client.GameStarted += (difficulty, mode, _, duration) => received.Add(difficulty);
                await client.ConnectAsync();
                await client.ConnectAsync();

                Assert.That(received, Has.Count.EqualTo(1));
            }
            finally { await client.DisconnectAsync(); }
        }

        [Test]
        public async Task 切断したあとに購読しても開始は流れない()
        {
            var client = new FakeGameMasterClient(NullLogger<FakeGameMasterClient>.Instance);
            var received = new List<Difficulty>();

            await client.ConnectAsync();
            await client.DisconnectAsync();
            client.GameStarted += (difficulty, mode, _, duration) => received.Add(difficulty);

            Assert.That(received, Is.Empty);
        }

        [Test]
        public async Task 購読を外すと届かなくなる()
        {
            var client = new FakeGameMasterClient(NullLogger<FakeGameMasterClient>.Instance);
            var received = new List<Difficulty>();
            void Handler(Difficulty d, GameMode mode, PlayerId _, TimeSpan duration) => received.Add(d);

            try
            {
                client.GameStarted += Handler;
                client.GameStarted -= Handler;
                await client.ConnectAsync();

                Assert.That(received, Is.Empty);
            }
            finally { await client.DisconnectAsync(); }
        }
        [Test]
        public async Task Combo_bonuses_apply_at_thresholds_and_restart_after_reset()
        {
            var client = new FakeGameMasterClient(NullLogger<FakeGameMasterClient>.Instance);
            var combos = new List<uint>();
            var scores = new List<uint>();
            var changes = new List<ScoreChange>();
            var id = Guid.Empty;
            client.TargetAdded += target => id = target.Id;
            client.ComboChanged += combos.Add;
            try
            {
                Assert.That(await client.ResetComboAsync(), Is.False);
                await client.ConnectAsync();
                client.ScoreChanged += change =>
                {
                    scores.Add(change.TotalScore);
                    changes.Add(change);
                };
                await client.AddTargetAsync(default, 200);
                Assert.That(await client.ResetComboAsync(), Is.True);
                for (var i = 0; i < 6; i++) Assert.That(await client.HitTargetAsync(id), Is.True);
                Assert.That(combos, Is.EqualTo(new uint[] { 1, 2, 3, 4, 5, 6 }));
                Assert.That(scores, Is.EqualTo(new uint[] { 500, 1000, 1500, 2000, 2500, 3000, 3800, 4300 }));
                Assert.That(changes[3].Reason, Is.EqualTo(ScoreReason.Combo));
                Assert.That(changes[3].Difference, Is.EqualTo(500));
                Assert.That(changes[6].Reason, Is.EqualTo(ScoreReason.Combo));
                Assert.That(changes[6].Difference, Is.EqualTo(800));
                Assert.That(changes[0].TargetId, Is.EqualTo(id));
                Assert.That(await client.ResetComboAsync(), Is.True);
                Assert.That(await client.ResetComboAsync(), Is.True);
                Assert.That(await client.HitTargetAsync(id), Is.True);
                Assert.That(combos, Is.EqualTo(new uint[] { 1, 2, 3, 4, 5, 6, 0, 1 }));
                Assert.That(scores[scores.Count - 1], Is.EqualTo(4800));
            }
            finally { await client.DisconnectAsync(); }
        }

        [Test]
        public async Task Registered_targets_score_repeatedly_until_removed()
        {
            var client = new FakeGameMasterClient(NullLogger<FakeGameMasterClient>.Instance);
            var scores = new List<uint>();
            client.ScoreChanged += change => scores.Add(change.TotalScore);
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
            var client = new FakeGameMasterClient(NullLogger<FakeGameMasterClient>.Instance);
            try
            {
                await client.ConnectAsync();
                var scores = new List<uint>();
                client.ScoreChanged += change => scores.Add(change.TotalScore);
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
            var client = new FakeGameMasterClient(NullLogger<FakeGameMasterClient>.Instance);
            var events = new List<string>();
            client.TargetAdded += _ => events.Add("added");
            client.TargetRemoved += _ => events.Add("removed");
            client.GameStarted += (_, mode, __, duration) => events.Add("started");
            try
            {
                await client.ConnectAsync();
                await client.AddTargetAsync(default, 200);
                events.Clear();
                await client.DisconnectAsync();
                Assert.That(events, Is.Empty);
                await client.ConnectAsync();
                Assert.That(events, Is.EqualTo(new[] { "removed", "removed", "started" }));
            }
            finally { await client.DisconnectAsync(); }
        }

        [Test]
        public async Task Game_finished_does_not_emit_target_removals()
        {
            var client = new FakeGameMasterClient(TimeSpan.FromMilliseconds(250), TimeSpan.Zero, NullLogger<FakeGameMasterClient>.Instance);
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
            var client = new FakeGameMasterClient(TimeSpan.Zero, TimeSpan.Zero, NullLogger<FakeGameMasterClient>.Instance);
            var events = new List<string>();
            var playerId = default(PlayerId);
            var gameId = default(GameId);
            client.GameStarted += (_, mode, player, duration) => { playerId = player; events.Add("started"); };
            client.GameFinished += game => { gameId = game; events.Add("finished"); };
            client.RankingScreenRequested += () => events.Add("ranking");
            client.HomeScreenRequested += () => events.Add("home");
            try
            {
                await client.ConnectAsync();
                Assert.That(events, Is.EqualTo(new[] { "started", "finished", "ranking", "home" }));
                Assert.That(gameId.Value, Is.GreaterThan(0));
                Assert.That(await client.GetGameResultAsync(gameId), Is.EqualTo(0));
                var ranking = await client.GetRankingAsync(10, GameMode.Single, Difficulty.Normal);
                var history = await client.GetGamesByPlayerAsync(playerId, GameMode.Single, Difficulty.Normal);
                Assert.That(ranking.Count, Is.EqualTo(10));
                Assert.That(history, Has.Some.Matches<RankingEntry>(game => game.GameId.Equals(gameId) && game.Rank == 13));
                Assert.That(await client.GetGameResultAsync(new GameId(9999)), Is.Null);
            }
            finally { await client.DisconnectAsync(); }
        }

        [Test]
        public async Task Pocket_watch_is_created_and_replaced_by_server_events()
        {
            var client = new FakeGameMasterClient(NullLogger<FakeGameMasterClient>.Instance);
            var watches = new List<Guid>();
            var removed = new List<Guid>();
            client.PocketWatchAdded += (id, owner) => watches.Add(id);
            client.TargetRemoved += removed.Add;
            try
            {
                await client.ConnectAsync();
                Assert.That(watches, Has.Count.EqualTo(1));
                var original = watches[0];
                Assert.That(await client.HitTargetAsync(original), Is.True);
                Assert.That(removed, Is.EqualTo(new[] { original }));
                Assert.That(watches, Has.Count.EqualTo(2));
                Assert.That(watches[1], Is.Not.EqualTo(original));
                Assert.That(await client.HitTargetAsync(original), Is.False);
            }
            finally { await client.DisconnectAsync(); }
        }

    }
}
