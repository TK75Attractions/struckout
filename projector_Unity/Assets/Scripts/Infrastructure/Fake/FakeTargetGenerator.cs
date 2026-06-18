using Struckout.Application;
using Struckout.Domain;
using System;
using System.Collections.Generic;

namespace Struckout.Infrastructure
{
    public class FakeTargetGenerator : ITargetGenerator
    {
        public IReadOnlyList<Target> GenerateTargets(int num)
        {
            List<Target> result = new();
            for (int i = 0; i < num; i++)
            {
                var target = CreateTarget(TargetType.Circle, i, i, MathF.Sqrt(2));
            
                result.Add(target);
            }

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