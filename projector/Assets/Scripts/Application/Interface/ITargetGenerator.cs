using Struckout.Domain;
using System.Collections.Generic;

namespace Struckout.Application
{
    public interface ITargetGenerator
    {
        Target GenerateTarget(TargetType type, IReadOnlyList<Target> existTarget);
        IReadOnlyList<Target> GenerateTargets(int num, TargetType type, IReadOnlyList<Target> existTarget);
    }
}