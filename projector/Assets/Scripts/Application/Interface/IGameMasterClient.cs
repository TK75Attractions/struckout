using System;
using System.Collections.Generic;
using Struckout.Domain;
using System.Threading;
using Cysharp.Threading.Tasks;
using Struckout.Domain.TargetDefinition;
using Tk75Attractions.Struckout.GameMaster.V1;
using Target = Struckout.Domain.TargetDefinition.Target;

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
        event Action<Difficulty, PlayerId> GameStarted;

        /// <summary>
        /// 制限時間が尽きてゲームが終わった。これ以降、的の操作は受け付けられない。
        /// GameStarted と同じく受信スレッドから飛ぶ。
        /// </summary>
        event Action<GameId> GameFinished;

        event Action RankingScreenRequested;
        event Action HomeScreenRequested;

        /// <summary>サーバーの残り時間。受信スレッドから通知されるため、UI 更新はマーシャリングする。</summary>
        event Action<TimeSpan> RemainingTimeChanged;

        /// <summary>サーバーの累計得点。受信スレッドから通知されるため、UI 更新はマーシャリングする。</summary>
        event Action<uint> ScoreChanged;

        /// <summary>登録された的。受信スレッドから通知されるため、状態・描画の更新はマーシャリングする。</summary>
        event Action<Target> TargetAdded;

        /// <summary>削除された的の UUID。受信スレッドから通知される。</summary>
        event Action<Guid> TargetRemoved;

        /// <summary>イベント購読が切れた。</summary>
        event Action ConnectionLost;

        /// <summary>接続してイベントの購読を始める。</summary>
        UniTask<bool> ConnectAsync();

        /// <summary>円形の的の登録を要求する。状態への追加は TargetAdded の受信時に行う。</summary>
        UniTask<bool> AddTargetAsync(TargetCoordinate coordinate, double radius, CancellationToken token = default);

        /// <summary>的の削除を要求する。状態からの削除は TargetRemoved の受信時に行う。</summary>
        UniTask<bool> RemoveTargetAsync(Guid targetId, CancellationToken token = default);

        /// <summary>命中を通知する。得点はサーバーが計算して ScoreChanged で通知する。</summary>
        UniTask<bool> HitTargetAsync(Guid targetId, CancellationToken token = default);

        UniTask<uint?> GetGameResultAsync(GameId gameId, CancellationToken token = default);
        UniTask<IReadOnlyList<RankingEntry>> GetRankingAsync(uint maxItemCount, CancellationToken token = default);
        UniTask<IReadOnlyList<PlayerGame>> GetGamesByPlayerAsync(PlayerId playerId, CancellationToken token = default);

        UniTask DisconnectAsync();
    }
}
