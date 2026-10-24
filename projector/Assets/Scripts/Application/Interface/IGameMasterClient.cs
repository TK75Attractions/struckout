using System;
using System.Threading;
using System.Threading.Tasks;
using Struckout.Domain.TargetDefinition;
using Tk75Attractions.Struckout.GameMaster.V1;

namespace Struckout.Application.Interface
{
    /// <summary>
    /// game_master との通信。gRPC で話す。
    ///
    /// projector の責務は的の登録・ヒット判定・描画で、
    /// ゲームの進行 (開始・終了・残り時間) は game_master から流れてくるイベントに従う。
    /// </summary>
    public interface IGameMasterClient
    {
        /// <summary>
        /// ゲームが始まった。この後で的を登録し、命中を通知できる。
        /// 受信スレッドから飛ぶので、UI に触る購読側はマーシャリングすること。
        /// </summary>
        event Action<Difficulty> GameStarted;

        /// <summary>
        /// 制限時間が尽きてゲームが終わった。これ以降、的の操作は受け付けられない。
        /// GameStarted と同じく受信スレッドから飛ぶ。
        /// </summary>
        event Action GameFinished;

        /// <summary>サーバーの残り時間。受信スレッドから通知されるため、UI 更新はマーシャリングする。</summary>
        event Action<TimeSpan> RemainingTimeChanged;

        /// <summary>サーバーの累計得点。受信スレッドから通知されるため、UI 更新はマーシャリングする。</summary>
        event Action<uint> ScoreChanged;

        /// <summary>イベント購読が切れた。</summary>
        event Action ConnectionLost;

        /// <summary>接続してイベントの購読を始める。</summary>
        Task<bool> ConnectAsync();

        /// <summary>円形の的を登録する。所有者はサーバーが決め、失敗時は null を返す。</summary>
        Task<Guid?> AddTargetAsync(TargetCoordinate coordinate, double radius, CancellationToken token = default);

        /// <summary>登録済みの的を削除する。移動時は削除後に新しい位置で登録する。</summary>
        Task<bool> RemoveTargetAsync(Guid targetId, CancellationToken token = default);

        /// <summary>命中を通知する。得点はサーバーが計算して ScoreChanged で通知する。</summary>
        Task<bool> HitTargetAsync(Guid targetId, CancellationToken token = default);

        Task DisconnectAsync();
    }
}
