using Struckout.Application;
using Struckout.Domain;
using System;
using System.Collections.Generic;

namespace Struckout.Infrastructure
{
    public class FakeTargetGenerator : ITargetGenerator
    {
        public IReadOnlyList<Target> GenerateTargets(int num, float size)
        {
            List<Target> result = new();
            for (int i = 0; i < num; i++)
            {
                var target = GenerateTarget(TargetType.Circle, i, i, size);
            
                result.Add(target);
            }

            return result;
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