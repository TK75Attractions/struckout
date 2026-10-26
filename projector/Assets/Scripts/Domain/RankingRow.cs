namespace Struckout.Domain
{
    public readonly struct RankingRow
    {
        public GameId GameId { get; }
        public uint Score { get; }
        public string PlayerName { get; }
        public bool IsPersonal { get; }
        public bool IsCurrentGame { get; }

        public RankingRow(GameId gameId, uint score, string playerName, bool isPersonal, bool isCurrentGame)
        {
            GameId = gameId;
            Score = score;
            PlayerName = playerName;
            IsPersonal = isPersonal;
            IsCurrentGame = isCurrentGame;
        }
    }
}
