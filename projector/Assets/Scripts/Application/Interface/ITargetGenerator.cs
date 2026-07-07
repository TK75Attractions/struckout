using Struckout.Domain;
using System.Collections.Generic;

namespace Struckout.Application
{
    public interface ITargetGenerator
    {
        IReadOnlyList<Target> GenerateTargets(int num, float size);
        IReadOnlyList<Target> GenerateTargets(int num, float size, IReadOnlyList<Target> existTarget);
        Target GenerateTarget(TargetType targetType,float X, float Y, float size);
    }
}