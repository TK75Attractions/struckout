using Struckout.Domain;
using System.Collections.Generic;

namespace Struckout.Application
{
    public interface IUIService
    {
        void InstantiateTargets(IReadOnlyList<Target> targets);
        /// <summary>
        /// 的に当たったときの見た目の更新。的は消えず、
        /// <paramref name="cooldownSeconds"/> の間だけ色を変えて当たらないことを示す。
        /// </summary>
        void OnTargetHit(Target target, float cooldownSeconds);

        /// <summary>
        /// ボールが当たった位置を描画座標で受け取り、その場にマーカーを出す。
        /// 当たり判定に関係なく呼ばれる。外れた位置が見えないと変換係数のずれに気づけないため。
        /// </summary>
        void ShowCollisionMarker(float x, float y, CollisionResult result);
    }
}