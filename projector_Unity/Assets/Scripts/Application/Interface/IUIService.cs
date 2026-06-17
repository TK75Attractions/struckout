using Struckout.Domain;
using System.Collections.Generic;

namespace Struckout.Application
{
    public interface IUIService
    {
        void InstantiateTargets(IReadOnlyList<Target> targets);
        void OnCollisionTarget(Target target);
    }
}