using System;
using System.Linq;
using NUnit.Framework;
using Struckout.Domain;
using Struckout.Domain.Ranking;

namespace Struckout.Tests.Tests.EditMode
{
    public class RankingResultsTests
    {
        [Test]
        public void Equal_scores_share_competition_ranks_and_skip_the_following_place()
        {
            var ranking = new[] { 100u, 90u, 90u, 80u }.Select((score, i) =>
                new RankingEntry(new GameId((uint)i + 1), score, new PlayerId(7), "本人")).Reverse().ToArray();
            var rows = RankingResults.Merge(ranking, ranking, new PlayerId(7), new GameId(3));
            Assert.That(rows.Select(row => row.Rank), Is.EqualTo(new uint?[] { 1, 2, 2, 4 }));
            Assert.That(RankingResults.GetRank(ranking, new GameId(3)), Is.EqualTo(2));
            Assert.That(RankingResults.GetRank(ranking, new GameId(99)), Is.Null);
        }

        [Test]
        public void History_uses_server_ranks_beyond_the_top_ten()
        {
            var ranking = Enumerable.Range(1, 10).Select(i =>
                new RankingEntry(new GameId((uint)i), (uint)(100 - i), new PlayerId(8), "別の人")).ToArray();
            var history = new[]
            {
                ranking[0],
                new RankingEntry(new GameId(11), 89, 11, new RankingParticipant(1, new PlayerId(7), "本人")),
                new RankingEntry(new GameId(12), 50, 12, new RankingParticipant(1, new PlayerId(7), "本人")),
            };
            var rows = RankingResults.Merge(ranking, history, new PlayerId(7), new GameId(11));
            Assert.That(rows.Take(10).Select(row => row.Rank), Is.EqualTo(Enumerable.Range(1, 10)));
            Assert.That(rows.Skip(10).Select(row => row.Rank), Is.EqualTo(new uint?[] { 11, 12 }));
            Assert.That(RankingResults.GetRank(ranking.Concat(history).ToArray(), new GameId(11)), Is.Null);
            Assert.That(RankingResults.GetRank(Array.Empty<RankingEntry>(), new GameId(1)), Is.Null);
        }

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
                new RankingEntry(new GameId(1), 100, new PlayerId(7), "本人"), new RankingEntry(new GameId(3), 150, new PlayerId(7), "本人"),
                new RankingEntry(new GameId(4), 90, new PlayerId(7), "本人"), new RankingEntry(new GameId(3), 150, new PlayerId(7), "本人"),
            };
            var rows = RankingResults.Merge(ranking, history, player, new GameId(3));
            Assert.That(rows.Select(row => row.GameId.Value), Is.EqualTo(new uint[] { 3, 1, 2, 4 }));
            Assert.That(rows.Where(row => row.IsPersonal).Select(row => row.GameId.Value), Is.EqualTo(new uint[] { 3, 1, 4 }));
            Assert.That(rows.Single(row => row.IsCurrentGame).GameId, Is.EqualTo(new GameId(3)));
            Assert.That(rows[1].PlayerName, Is.EqualTo("本人"));
        }

        [TestCase(7u)]
        [TestCase(8u)]
        public void Multi_play_ranking_and_history_show_both_names_and_highlight_either_participant(uint playerId)
        {
            var participants = new[]
            {
                new RankingParticipant(1, new PlayerId(7), "player1"),
                new RankingParticipant(2, new PlayerId(8), "player2"),
            };
            var ranking = new[] { new RankingEntry(new GameId(1), 100, participants) };
            var history = new[]
            {
                new RankingEntry(new GameId(1), 100, participants),
                new RankingEntry(new GameId(2), 50, participants),
            };
            var rows = RankingResults.Merge(ranking, history, new PlayerId(playerId), new GameId(2));
            Assert.That(rows.Count, Is.EqualTo(2));
            Assert.That(rows.All(row => row.IsPersonal), Is.True);
            Assert.That(rows.Select(row => row.PlayerName), Is.EqualTo(new[] { "player1 / player2", "player1 / player2" }));
            Assert.That(rows[1].IsCurrentGame, Is.True);
        }

        [Test]
        public void Limits_the_sorted_union_to_twelve_games_after_deduplication()
        {
            var player = new PlayerId(7);
            var ranking = Enumerable.Range(1, 10).Select(i =>
                new RankingEntry(new GameId((uint)i), (uint)(2000 - i * 100), new PlayerId(8), "別の人")).ToArray();
            var history = new[]
            {
                new RankingEntry(new GameId(1), 1900, new PlayerId(7), "本人"), new RankingEntry(new GameId(11), 1500, new PlayerId(7), "本人"),
                new RankingEntry(new GameId(12), 500, new PlayerId(7), "本人"), new RankingEntry(new GameId(13), 400, new PlayerId(7), "本人"),
                new RankingEntry(new GameId(14), 300, new PlayerId(7), "本人"),
            };
            var rows = RankingResults.Merge(ranking, history, player, new GameId(13));
            Assert.That(rows.Count, Is.EqualTo(12));
            Assert.That(rows.Select(row => row.GameId.Value), Is.EqualTo(new uint[] { 1, 2, 3, 4, 5, 11, 6, 7, 8, 9, 10, 12 }));
            Assert.That(rows.Count(row => row.GameId.Equals(new GameId(1))), Is.EqualTo(1));
        }

        [Test]
        public void Empty_results_remain_empty()
        {
            Assert.That(RankingResults.Merge(Array.Empty<RankingEntry>(), Array.Empty<RankingEntry>(), new PlayerId(7), null), Is.Empty);
        }
    }
}
