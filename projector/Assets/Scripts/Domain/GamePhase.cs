namespace Struckout.Domain
{
    /// <summary>
    /// projector から見たゲームの進行状態。
    ///
    /// game_master (session.rs) は、セッションが始まっていないのに得点が届くと panic する。
    /// そのため <see cref="Playing"/> の間だけ得点を送る。
    /// </summary>
    public enum GamePhase
    {
        /// <summary>StartGame を待っている。当たっても得点にしない。</summary>
        Idle = 0,

        /// <summary>プレイ中。</summary>
        Playing = 1,
    }
}
