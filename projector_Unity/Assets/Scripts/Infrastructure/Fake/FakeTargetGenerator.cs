using Struckout.Application;
using Struckout.Domain;
using System.Collections.Generic;

namespace Struckout.Infrastructure
{
    public class FakeTargetGenerator : ITargetGenerator
    {
        public List<Target> GenerateTargets(int num)
        {
            List<Target> result = new();
            var target = CreateTarget(TargetType.Circle, 1, 1, 1);
            
            result.Add(target);

            return result;
        }
        private Target CreateTarget(TargetType type, float X, float Y, float size)
        {
            TargetCoordinate coordinate = new(
                X,
                Y
            );
            Target target = new(
                coordinate,
                type,
                size
            );

            return target;
        }
    }
}