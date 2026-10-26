namespace Struckout.Domain
{
    public readonly struct RankingEntry
    {
        public GameId GameId { get; }
        public uint Score { get; }
        public PlayerId PlayerId { get; }
        public string PlayerName { get; }

        public RankingEntry(GameId gameId, uint score, PlayerId playerId, string playerName)
        {
            GameId = gameId;
            Score = score;
            PlayerId = playerId;
            PlayerName = playerName;
        }
    }
}
