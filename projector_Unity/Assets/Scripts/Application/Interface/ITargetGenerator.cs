using Struckout.Domain;
using System.Collections.Generic;

namespace Struckout.Application
{
    public interface ITargetGenerator
    {
        List<Target> GenerateTargets(int num);
    }
}