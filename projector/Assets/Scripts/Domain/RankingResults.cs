using System.Collections.Generic;
using System.Linq;

namespace Struckout.Domain
{
    public static class RankingResults
    {
        public const uint TopItemCount = 10;
        public const int DisplayItemCount = 12;

        /// <summary>Merges games by ID and selects the highest scores to fit the fixed result screen.</summary>
        public static IReadOnlyList<RankingRow> Merge(
            IReadOnlyList<RankingEntry> ranking, IReadOnlyList<PlayerGame> history,
            PlayerId playerId, GameId? currentGame)
        {
            var rows = new Dictionary<GameId, RankingRow>();
            foreach (var entry in ranking)
            {
                if (rows.TryGetValue(entry.GameId, out var existing) &&
                    (existing.IsPersonal || !entry.PlayerId.Equals(playerId))) continue;
                rows[entry.GameId] = new RankingRow(entry.GameId, entry.Score, entry.PlayerName,
                    entry.PlayerId.Equals(playerId), currentGame.HasValue && entry.GameId.Equals(currentGame.Value));
            }
            foreach (var game in history)
            {
                rows.TryGetValue(game.GameId, out var ranked);
                rows[game.GameId] = new RankingRow(game.GameId,
                    rows.ContainsKey(game.GameId) ? ranked.Score : game.Score,
                    ranked.IsPersonal ? ranked.PlayerName : null, true, currentGame.HasValue && game.GameId.Equals(currentGame.Value));
            }
            return rows.Values.OrderByDescending(row => row.Score).ThenBy(row => row.GameId.Value).Take(DisplayItemCount).ToArray();
        }
    }
}
