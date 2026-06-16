using Struckout.Domain;

namespace Struckout.Unity
{
    public interface ITargetUI
    {
        void Initialize(Target target);
        void OnCollision();
    }
}