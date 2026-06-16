using Struckout.Domain;
using System.Collections.Generic;

namespace Struckout.Application
{
    public interface IUIService
    {
        void InstantinateTargets(IReadOnlyList<Target> targets);
        void OnCollisionTarget(Target target);
    }
}