using System.Collections.Generic;
using Struckout.Application.Interface;
using Struckout.Domain.TargetDefinition;

namespace Struckout.Infrastructure.Fake
{
    public class FakeTargetGenerator : ITargetGenerator
    {
        public IReadOnlyList<TargetGeometry> GenerateTargets(int num, TargetType type, IReadOnlyList<TargetGeometry> targets)
        {
            List<TargetGeometry> result = new();
            for (var i = 0; i < num; i++)
            {
                var target = GenerateTarget(TargetType.Compass, i * 50, i * 50, 50);

                result.Add(target);
            }

            return result;
        }

        public TargetGeometry GenerateTarget(TargetType type, IReadOnlyList<TargetGeometry> targets)
        {
            return GenerateTarget(TargetType.Compass, 50, 50, 50);
        }

        /// <summary>ダミーなので、決まった場所に置き直すだけ。</summary>
        public TargetCoordinate PickRelocation(TargetGeometry target, IReadOnlyList<TargetGeometry> others)
            => new(100f, 100f);

        public TargetGeometry GenerateTarget(TargetType type, float x, float y, float size)
        {
            TargetCoordinate coordinate = new(
                x,
                y
            );
            TargetGeometry target = new(
                coordinate,
                type,
                size
            );

            return target;
        }
    }
}