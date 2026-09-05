using Struckout.Domain;
using System.Collections.Generic;

namespace Struckout.Application
{
    public interface ITargetGenerator
    {
        Target GenerateTarget(TargetType type, IReadOnlyList<Target> existTarget);
        IReadOnlyList<Target> GenerateTargets(int num, TargetType type, IReadOnlyList<Target> existTarget);

        /// <summary>
        /// 当たった的の移動先を選ぶ。難易度を問わず、当たった的はその場に留まらない。
        ///
        /// 初期配置と違って乱数で決める。決定的に選ぶと、空いた場所が必ず最も条件のよい点になり、
        /// 撃った的が同じ場所に戻ってきてしまうため。
        /// </summary>
        /// <param name="target">動かす的。<paramref name="others"/> には含めないこと。</param>
        /// <param name="others">避けたい他の的。</param>
        TargetCoordinate PickRelocation(Target target, IReadOnlyList<Target> others);
    }
}
