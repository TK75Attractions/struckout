using System;
using System.Threading.Tasks;
using Tk75Attractions.Struckout.V1;

namespace Struckout.Application
{
    /// <summary>
    /// game_master との通信。gRPC で話す。
    ///
    /// projector の責務はヒット判定・描画・点数計上だけで、
    /// ゲームの進行 (開始・終了・残り時間) は game_master から流れてくるイベントに従う。
    /// </summary>
    public interface IGameMasterClient
    {
        /// <summary>
        /// ゲームが始まった。難易度と game_id はこのイベントでしか手に入らない。
        /// 受信スレッドから飛ぶので、UI に触る購読側はマーシャリングすること。
        /// </summary>
        event Action<Event.Types.GameStarted> GameStarted;

        /// <summary>イベント購読が切れた。</summary>
        event Action ConnectionLost;

        /// <summary>接続してイベントの購読を始める。</summary>
        Task<bool> ConnectAsync();

        /// <summary>
        /// 得点を加算する。game_master 側が加算するので送るのは差分。
        /// ゲームが始まっていなければ送らずに false を返す。
        /// </summary>
        Task<bool> AddScoreAsync(int scoreToAdd);

        Task DisconnectAsync();
    }
}
