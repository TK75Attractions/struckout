using System.Collections.Generic;
using System.Linq;

namespace Struckout.Domain.Ranking
{
    public readonly struct RankingParticipant
    {
        public uint MachineId { get; }
        public PlayerId PlayerId { get; }
        public string PlayerName { get; }

        public RankingParticipant(uint machineId, PlayerId playerId, string playerName)
        {
            MachineId = machineId;
            PlayerId = playerId;
            PlayerName = playerName;
        }
    }

    public readonly struct RankingEntry
    {
        public GameId GameId { get; }
        public uint Score { get; }
        public IReadOnlyList<RankingParticipant> Participants { get; }
        public uint? Rank { get; }
        public string PlayerName => string.Join(" / ", Participants.Select(player => player.PlayerName));

        public RankingEntry(GameId gameId, uint score, params RankingParticipant[] participants)
            : this(gameId, score, null, participants) { }

        public RankingEntry(GameId gameId, uint score, uint? rank, params RankingParticipant[] participants)
        {
            GameId = gameId;
            Score = score;
            Rank = rank;
            Participants = System.Array.AsReadOnly((RankingParticipant[])participants.Clone());
        }

        public RankingEntry(GameId gameId, uint score, PlayerId playerId, string playerName)
            : this(gameId, score, new RankingParticipant(1, playerId, playerName)) { }
    }
}
