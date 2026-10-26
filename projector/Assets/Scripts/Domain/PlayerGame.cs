namespace Struckout.Domain
{
    public readonly struct PlayerGame
    {
        public GameId GameId { get; }
        public uint Score { get; }
        public PlayerGame(GameId gameId, uint score) { GameId = gameId; Score = score; }
    }
}
