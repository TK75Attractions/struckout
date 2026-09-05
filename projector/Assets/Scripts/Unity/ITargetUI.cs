using Struckout.Domain;

namespace Struckout.Unity
{
    public interface ITargetUI
    {
        void Initialize(Target target);

        /// <summary>
        /// 的が動いたときに呼ぶ。<paramref name="target"/> は既に新しい座標を持っている。
        /// 的は消えないので、同じ GameObject がそのまま移る。
        /// </summary>
        void MoveTo(Target target);
    }
}
