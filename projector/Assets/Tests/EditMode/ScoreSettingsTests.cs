using NUnit.Framework;
using Struckout.Domain;
using Struckout.Infrastructure;

namespace Struckout.Tests
{
    /// <summary>
    /// 点数の決め方を固定する。
    ///
    /// 検証しているのは規則であって、既定値そのものではない。
    /// 点数はまだ確定しておらず Inspector で差し替える前提なので、
    /// 値を焼き付けるとチューニングのたびにテストが落ちてしまう。
    /// </summary>
    public class ScoreSettingsTests
    {
        private static ScoreSettings.SizeTier Tier(float maxDiameter, int points) =>
            new() { MaxDiameter = maxDiameter, Points = points };

        private static Target Circle(float diameter) =>
            new(new TargetCoordinate(0f, 0f), TargetType.Circle, diameter);

        [Test]
        public void 段に収まる的はその段の点数になる()
        {
            var settings = new ScoreSettings
            {
                Tiers = new[] { Tier(100f, 5) },
                DefaultPoints = 1,
            };

            Assert.That(settings.PointsFor(50f), Is.EqualTo(5));
        }

        [Test]
        public void 境界ちょうどはその段に含まれる()
        {
            var settings = new ScoreSettings
            {
                Tiers = new[] { Tier(100f, 5) },
                DefaultPoints = 1,
            };

            Assert.That(settings.PointsFor(100f), Is.EqualTo(5));
        }

        [Test]
        public void どの段にも収まらない的は既定の点数になる()
        {
            var settings = new ScoreSettings
            {
                Tiers = new[] { Tier(100f, 5) },
                DefaultPoints = 1,
            };

            Assert.That(settings.PointsFor(101f), Is.EqualTo(1));
        }

        [Test]
        public void 収まる段が複数あるときはいちばん狭い段が選ばれる()
        {
            var settings = new ScoreSettings
            {
                Tiers = new[] { Tier(300f, 3), Tier(150f, 5) },
                DefaultPoints = 1,
            };

            Assert.That(settings.PointsFor(100f), Is.EqualTo(5), "150 の段が 300 の段より優先される");
            Assert.That(settings.PointsFor(200f), Is.EqualTo(3), "150 に収まらなければ 300 の段");
        }

        [Test]
        public void 段の並び順は結果に影響しない()
        {
            var ascending = new ScoreSettings
            {
                Tiers = new[] { Tier(150f, 5), Tier(300f, 3) },
                DefaultPoints = 1,
            };
            var descending = new ScoreSettings
            {
                Tiers = new[] { Tier(300f, 3), Tier(150f, 5) },
                DefaultPoints = 1,
            };

            foreach (var diameter in new[] { 100f, 200f, 400f })
            {
                Assert.That(
                    ascending.PointsFor(diameter),
                    Is.EqualTo(descending.PointsFor(diameter)),
                    $"直径 {diameter} で並び順によって差が出ている");
            }
        }

        [Test]
        public void 段が無ければ既定の点数になる()
        {
            var settings = new ScoreSettings { Tiers = null, DefaultPoints = 7 };

            Assert.That(settings.PointsFor(100f), Is.EqualTo(7));
        }

        [Test]
        public void 小さい的ほど高得点という既定の向きが保たれる()
        {
            // 既定値そのものではなく、既定値が表している向きだけを固定する。
            var settings = new ScoreSettings();

            Assert.That(
                settings.PointsFor(100f),
                Is.GreaterThanOrEqualTo(settings.PointsFor(1000f)),
                "小さい的のほうが低い点数になっている");
        }

        [Test]
        public void 負の点数は送らない()
        {
            // 得点は差分として game_master に送られ、プロトコル上は符号なし。
            var settings = new ScoreSettings
            {
                Tiers = new[] { Tier(100f, -5) },
                DefaultPoints = 1,
            };

            Assert.That(settings.PointsFor(50f), Is.EqualTo(0));
        }

        [Test]
        public void PointCalculator_は的の直径で点数を引く()
        {
            var settings = new ScoreSettings
            {
                Tiers = new[] { Tier(100f, 5) },
                DefaultPoints = 1,
            };
            var calculator = new PointCalculator(settings);

            Assert.That(calculator.CalculatePoint(Circle(50f)), Is.EqualTo(5));
            Assert.That(calculator.CalculatePoint(Circle(500f)), Is.EqualTo(1));
        }
    }
}
