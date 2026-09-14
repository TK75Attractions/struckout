using Struckout.Domain;

namespace Struckout.Unity
{
    public interface ITargetUI
    {
        /// <summary>
        /// 生成直後に一度だけ呼ぶ。
        /// <paramref name="world"/> は盤面座標 (px) をワールド座標に直すのに使い、
        /// 以降の <see cref="MoveTo"/> でも同じものを使いまわす。
        /// </summary>
        void Initialize(Target target, WorldCoordinateTransform world);

        /// <summary>
        /// 的が動いたときに呼ぶ。<paramref name="target"/> は既に新しい座標を持っている。
        /// 的は消えないので、同じ GameObject がそのまま移る。
        /// </summary>
        void MoveTo(Target target);
    }
}
