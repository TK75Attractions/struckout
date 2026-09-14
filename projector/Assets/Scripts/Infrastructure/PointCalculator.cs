using System;
using Struckout.Application;
using Struckout.Domain;

namespace Struckout.Infrastructure
{
    /// <summary>
    /// 的の大きさから点数を決める。
    ///
    /// 点数そのものは <see cref="ScoreSettings"/> が持つ。ここには規則しか無いので、
    /// 値の調整でこのクラスを触る必要はない。
    /// </summary>
    public class PointCalculator : IPointCalculator
    {
        private readonly ScoreSettings _settings;

        public PointCalculator(ScoreSettings settings)
        {
            _settings = settings ?? throw new ArgumentNullException(nameof(settings));
        }

        public int CalculatePoint(Target target)
        {
            if (target == null) throw new ArgumentNullException(nameof(target));

            return _settings.PointsFor(target.Diameter);
        }
    }
}
