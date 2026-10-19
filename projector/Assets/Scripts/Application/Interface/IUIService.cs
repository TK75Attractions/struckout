using System.Collections.Generic;
using System;
using Struckout.Domain;
using Struckout.Domain.TargetDefinition;

namespace Struckout.Application.Interface
{
    public interface IUIService
    {
        /// <summary>投影画面をゲーム進行状態に合わせて切り替える。</summary>
        void SetGamePhase(GamePhase phase);

        /// <summary>game_master から受信した残り時間を表示する。</summary>
        void SetRemainingTime(TimeSpan remaining);

        /// <summary>game_master が確定した累計得点を表示する。</summary>
        void SetScore(uint totalScore);

        void InstantiateTargets(IReadOnlyList<Target> targets);
        /// <summary>
        /// 的が移動したことを見た目に反映する。的は消えない。
        /// 座標は <paramref name="target"/> が既に持っている。
        /// </summary>
        void MoveTarget(Target target);

        /// <summary>
        /// ボールが当たった位置を描画座標で受け取り、その場にマーカーを出す。
        /// 当たり判定に関係なく呼ばれる。外れた位置が見えないと変換係数のずれに気づけないため。
        /// </summary>
        void ShowCollisionMarker(float x, float y, CollisionResult result);
    }
}
