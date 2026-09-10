using Struckout.Application;
using Struckout.Domain;
using System.Collections.Generic;

namespace Struckout.Infrastructure
{
    public class FakeTargetGenerator : ITargetGenerator
    {
        public IReadOnlyList<Target> GenerateTargets(int num, TargetType type, IReadOnlyList<Target> targets)
        {
            List<Target> result = new();
            for (int i = 0; i < num; i++)
            {
                var target = GenerateTarget(TargetType.Circle, i*50, i*50, 50);
            
                result.Add(target);
            }

            return result;
        }

        public Target GenerateTarget(TargetType type, IReadOnlyList<Target> targets)
        {
            return GenerateTarget(TargetType.Circle, 50, 50, 50);
        }

        public Target GenerateTarget(TargetType type, float X, float Y, float size)
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