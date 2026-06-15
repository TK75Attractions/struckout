using Struckout.Domain;
using System.Collections.Generic;

namespace Struckout.Application
{
    public interface ITargetGenerator
    {
        IReadOnlyList<Target> GenerateTargets(int num);
    }
}