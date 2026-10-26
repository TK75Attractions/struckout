using System;
using System.Linq;
using NUnit.Framework;
using Struckout.Domain;

namespace Struckout.Tests.Tests.EditMode
{
    public class RankingResultsTests
    {
        [Test]
        public void Merges_by_game_id_sorts_scores_and_keeps_personal_games_outside_the_top_list()
        {
            var player = new PlayerId(7);
            var ranking = new[]
            {
                new RankingEntry(new GameId(1), 100, player, "本人"),
                new RankingEntry(new GameId(2), 90, new PlayerId(8), "別の人"),
            };
            var history = new[]
            {
                new PlayerGame(new GameId(1), 100), new PlayerGame(new GameId(3), 150),
                new PlayerGame(new GameId(4), 90), new PlayerGame(new GameId(3), 150),
            };
            var rows = RankingResults.Merge(ranking, history, player, new GameId(3));
            Assert.That(rows.Select(row => row.GameId.Value), Is.EqualTo(new uint[] { 3, 1, 2, 4 }));
            Assert.That(rows.Where(row => row.IsPersonal).Select(row => row.GameId.Value), Is.EqualTo(new uint[] { 3, 1, 4 }));
            Assert.That(rows.Single(row => row.IsCurrentGame).GameId, Is.EqualTo(new GameId(3)));
            Assert.That(rows[1].PlayerName, Is.EqualTo("本人"));
        }

        [TestCase(false)]
        [TestCase(true)]
        public void A_shared_game_keeps_the_current_players_name_regardless_of_ranking_row_order(bool reverse)
        {
            var ranking = new[]
            {
                new RankingEntry(new GameId(1), 100, new PlayerId(8), "別の人"),
                new RankingEntry(new GameId(1), 100, new PlayerId(7), "本人"),
            };
            if (reverse) Array.Reverse(ranking);
            var rows = RankingResults.Merge(ranking, Array.Empty<PlayerGame>(), new PlayerId(7), null);
            Assert.That(rows.Count, Is.EqualTo(1));
            Assert.That(rows[0].IsPersonal, Is.True);
            Assert.That(rows[0].PlayerName, Is.EqualTo("本人"));
        }

        [Test]
        public void Personal_history_does_not_present_a_different_players_name_as_the_current_player()
        {
            var rows = RankingResults.Merge(new[] { new RankingEntry(new GameId(1), 100, new PlayerId(8), "別の人") },
                new[] { new PlayerGame(new GameId(1), 100) }, new PlayerId(7), null);
            Assert.That(rows.Count, Is.EqualTo(1));
            Assert.That(rows[0].IsPersonal, Is.True);
            Assert.That(rows[0].PlayerName, Is.Null.Or.Empty);
        }

        [Test]
        public void Limits_the_sorted_union_to_twelve_games_after_deduplication()
        {
            var player = new PlayerId(7);
            var ranking = Enumerable.Range(1, 10).Select(i =>
                new RankingEntry(new GameId((uint)i), (uint)(2000 - i * 100), new PlayerId(8), "別の人")).ToArray();
            var history = new[]
            {
                new PlayerGame(new GameId(1), 1900), new PlayerGame(new GameId(11), 1500),
                new PlayerGame(new GameId(12), 500), new PlayerGame(new GameId(13), 400),
                new PlayerGame(new GameId(14), 300),
            };
            var rows = RankingResults.Merge(ranking, history, player, new GameId(13));
            Assert.That(rows.Count, Is.EqualTo(12));
            Assert.That(rows.Select(row => row.GameId.Value), Is.EqualTo(new uint[] { 1, 2, 3, 4, 5, 11, 6, 7, 8, 9, 10, 12 }));
            Assert.That(rows.Count(row => row.GameId.Equals(new GameId(1))), Is.EqualTo(1));
        }

        [Test]
        public void Empty_results_remain_empty()
        {
            Assert.That(RankingResults.Merge(Array.Empty<RankingEntry>(), Array.Empty<PlayerGame>(), new PlayerId(7), null), Is.Empty);
        }
    }
}
