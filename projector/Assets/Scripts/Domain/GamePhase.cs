namespace Struckout.Domain
{
    /// <summary>
    /// projector から見たゲームの進行状態。
    ///
    /// 得点を送ってよいのは <see cref="Playing"/> の間だけ。
    /// game_master はセッション外の得点を running_games に見つけられず、
    /// NotFound を返す (service.rs の add_score)。
    /// </summary>
    public enum GamePhase
    {
        /// <summary>StartGame を待っている。当たっても得点にしない。</summary>
        Idle = 0,

        /// <summary>プレイ中。</summary>
        Playing = 1,

        /// <summary>制限時間が尽きた。次の GameStarted まで得点にしない。</summary>
        Finished = 2,
    }
}
