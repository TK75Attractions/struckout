using Struckout.Domain;

namespace Struckout.Unity
{
    public interface ITargetUI
    {
        void Initialize(Target target);

        /// <summary>
        /// 当たったときの見た目。的は消えず、
        /// <paramref name="cooldownSeconds"/> の間だけ当たらない状態を示す。
        /// </summary>
        void OnCollision(float cooldownSeconds);
    }
}
