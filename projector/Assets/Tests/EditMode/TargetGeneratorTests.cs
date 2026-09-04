using System.Collections.Generic;
using NUnit.Framework;
using Struckout.Domain;
using Struckout.Infrastructure;

namespace Struckout.Tests
{
    /// <summary>
    /// 的の配置は乱数を使わず毎回同じになる。これは仕様。
    ///
    /// 将来は「意図した場所に毎回同じように配置する」か
    /// 「配置パターンを複数用意して切り替える」形にする予定なので、
    /// ここで現在の配置を固定しておき、意図しない変化に気づけるようにする。
    ///
    /// 配置を変える改修のときは、この期待値も一緒に更新すること。
    /// </summary>
    public class TargetGeneratorTests
    {
        private const float Tolerance = 0.1f;

        [Test]
        public void 初期配置は毎回同じになる()
        {
            var first = new TargetGenerator().GenerateTargets(4, TargetType.Circle, new List<Target>());
            var second = new TargetGenerator().GenerateTargets(4, TargetType.Circle, new List<Target>());

            Assert.That(second, Is.EqualTo(first));
        }

        [Test]
        public void 初期配置は既知の座標になる()
        {
            var targets = new TargetGenerator().GenerateTargets(4, TargetType.Circle, new List<Target>());

            Assert.That(targets.Count, Is.EqualTo(4));

            // 1 個目だけは既存の的が無く、GetScore が float.MaxValue を返すため
            // 画面端までの距離が効かない。結果として Halton 列の i=1 の点がそのまま採用される。
            AssertTarget(targets[0], 960.0f, 360.0f, 500.0f);
            AssertTarget(targets[1], 375.0f, 586.7f, 375.0f);
            AssertTarget(targets[2], 1545.0f, 617.8f, 375.0f);
            AssertTarget(targets[3], 795.0f, 844.4f, 235.6f);
        }

        [Test]
        public void 的どうしは重ならない()
        {
            var targets = new TargetGenerator().GenerateTargets(4, TargetType.Circle, new List<Target>());

            for (int i = 0; i < targets.Count; i++)
            {
                for (int j = i + 1; j < targets.Count; j++)
                {
                    float dx = targets[i].Coordinate.X - targets[j].Coordinate.X;
                    float dy = targets[i].Coordinate.Y - targets[j].Coordinate.Y;
                    float distance = Mathf(dx, dy);

                    Assert.That(distance, Is.GreaterThan(targets[i].Radius + targets[j].Radius),
                        $"的 {i} と {j} が重なっている");
                }
            }
        }

        [Test]
        public void 的は画面内に収まる()
        {
            var targets = new TargetGenerator().GenerateTargets(4, TargetType.Circle, new List<Target>());

            foreach (var target in targets)
            {
                Assert.That(target.Coordinate.X, Is.InRange(0f, 1920f));
                Assert.That(target.Coordinate.Y, Is.InRange(0f, 1080f));
            }
        }

        private static float Mathf(float dx, float dy) =>
            (float)System.Math.Sqrt(dx * dx + dy * dy);

        private static void AssertTarget(Target target, float x, float y, float diameter)
        {
            Assert.That(target.Coordinate.X, Is.EqualTo(x).Within(Tolerance));
            Assert.That(target.Coordinate.Y, Is.EqualTo(y).Within(Tolerance));
            Assert.That(target.Size, Is.EqualTo(diameter).Within(Tolerance));
        }
    }
}
