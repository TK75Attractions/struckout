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
            var rows = new Dictionary<GameId, RankingRow>();
            foreach (var entry in ranking.Concat(history))
            {
                rows[entry.GameId] = new RankingRow(entry.GameId, entry.Score, entry.PlayerName,
                    entry.Participants.Any(player => player.PlayerId.Equals(playerId)),
                    currentGame.HasValue && entry.GameId.Equals(currentGame.Value));
            }
            var sorted = rows.Values.OrderByDescending(row => row.Score).ThenBy(row => row.GameId.Value).Take(DisplayItemCount).ToArray();
            uint rank = 0;
            for (var i = 0; i < sorted.Length; i++)
            {
                if (i == 0 || sorted[i].Score != sorted[i - 1].Score) rank = (uint)i + 1;
                var row = sorted[i];
                sorted[i] = new RankingRow(row.GameId, row.Score, row.PlayerName, row.IsPersonal,
                    row.IsCurrentGame, i < TopItemCount ? rank : null);
            }
            return sorted;
        }

        /// <summary>Finds a game's competition rank within the known top rows; history below them has no global rank.</summary>
        public static uint? GetRank(IReadOnlyList<RankingEntry> ranking, GameId gameId)
        {
            return Merge(ranking, System.Array.Empty<RankingEntry>(), default, gameId)
                .FirstOrDefault(row => row.GameId.Equals(gameId)).Rank;
        }
    }
}
