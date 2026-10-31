using System.Collections.Generic;
using System.Linq;

namespace Struckout.Domain.Ranking
{
    public static class RankingResults
    {
        public const uint TopItemCount = 10;
        public const int DisplayItemCount = 12;

        /// <summary>Merges games by ID and selects the highest scores to fit the fixed result screen.</summary>
        public static IReadOnlyList<RankingRow> Merge(
            IReadOnlyList<RankingEntry> ranking, IReadOnlyList<RankingEntry> history,
            PlayerId playerId, GameId? currentGame)
        {
            var entries = new Dictionary<GameId, RankingEntry>();
            foreach (var entry in ranking.Concat(history))
                entries[entry.GameId] = entry;
            var sorted = entries.Values.OrderByDescending(entry => entry.Score)
                .ThenBy(entry => entry.GameId.Value).Take(DisplayItemCount).ToArray();
            var rows = new RankingRow[sorted.Length];
            uint rank = 0;
            for (var i = 0; i < sorted.Length; i++)
            {
                if (i == 0 || sorted[i].Score != sorted[i - 1].Score) rank = (uint)i + 1;
                var entry = sorted[i];
                rows[i] = new RankingRow(entry.GameId, entry.Score, entry.PlayerName,
                    entry.Participants.Any(player => player.PlayerId.Equals(playerId)),
                    currentGame.HasValue && entry.GameId.Equals(currentGame.Value),
                    entry.Rank ?? (i < TopItemCount ? rank : (uint?)null));
            }
            return rows;
        }

        /// <summary>Uses a server rank when available, including games outside the displayed rows.</summary>
        public static uint? GetRank(IReadOnlyList<RankingEntry> ranking, GameId gameId)
        {
            foreach (var entry in ranking)
                if (entry.GameId.Equals(gameId) && entry.Rank.HasValue) return entry.Rank;
            return Merge(ranking.Take((int)TopItemCount).ToArray(), System.Array.Empty<RankingEntry>(), default, gameId)
                .FirstOrDefault(row => row.GameId.Equals(gameId)).Rank;
        }
    }
}
