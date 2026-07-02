using Struckout.Domain;
using System.Collections.Generic;

namespace Struckout.Application
{
    public interface ITargetGenerator
    {
        IReadOnlyList<Target> GenerateTargets(int num, float size);
        Target GenerateTarget(TargetType targetType,float X, float Y, float size);
    }
}