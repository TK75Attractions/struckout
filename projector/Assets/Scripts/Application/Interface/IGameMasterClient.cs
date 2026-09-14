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
        /// ゲームが始まった。難易度はこのイベントでしか手に入らない。
        /// game_id は得点の送信にしか使わないので、実装側が内部で持つ。
        /// 受信スレッドから飛ぶので、UI に触る購読側はマーシャリングすること。
        /// </summary>
        event Action<Difficulty> GameStarted;

        /// <summary>
        /// 制限時間が尽きてゲームが終わった。これ以降 game_master は
        /// この game_id を running_games から外すので、得点を送っても弾かれる。
        /// GameStarted と同じく受信スレッドから飛ぶ。
        /// </summary>
        event Action GameFinished;

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
