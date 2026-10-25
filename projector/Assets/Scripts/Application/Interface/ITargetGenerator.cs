using System.Collections.Generic;
using Struckout.Domain.TargetDefinition;

namespace Struckout.Application.Interface
{
    public interface ITargetGenerator
    {
        TargetGeometry GenerateTarget(TargetType type, IReadOnlyList<TargetGeometry> existTarget);
        IReadOnlyList<TargetGeometry> GenerateTargets(int num, TargetType type, IReadOnlyList<TargetGeometry> existTarget);

        /// <summary>
        /// 当たった的を削除した後、新しい的を登録する位置を選ぶ。
        ///
        /// 初期配置と違って乱数で決める。決定的に選ぶと、空いた場所が必ず最も条件のよい点になり、
        /// 撃った的が同じ場所に戻ってきてしまうため。
        /// </summary>
        /// <param name="target">削除した的の形状と大きさ。<paramref name="others"/> には含めないこと。</param>
        /// <param name="others">避けたい他の的。</param>
        TargetCoordinate PickRelocation(TargetGeometry target, IReadOnlyList<TargetGeometry> others);
    }
}
