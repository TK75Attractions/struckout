using System.Collections.Generic;
using NUnit.Framework;
using Struckout.Domain;
using Struckout.Domain.TargetDefinition;
using Struckout.Domain.Targets;
using System;
using Difficulty = Tk75Attractions.Struckout.GameMaster.V1.Difficulty;

namespace Struckout.Tests.Tests.EditMode
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

        [TestCase(Difficulty.Normal, 0.0)]
        [TestCase(Difficulty.Hard, 100.0)]
        [TestCase(Difficulty.Veryhard, 200.0)]
        public void Difficulty_sets_compass_speed(Difficulty difficulty, double expectedSpeed)
        {
            var geometry = new TargetGeometry(new TargetCoordinate(500, 500), TargetType.Compass, 100);
            var moving = new TargetGenerator().WithMovement(geometry, difficulty);
            var velocity = moving.Velocity;
            Assert.That(Math.Sqrt(velocity.X * velocity.X + velocity.Y * velocity.Y),
                Is.EqualTo(expectedSpeed).Within(0.0001));
            Assert.That(moving.Coordinate, Is.EqualTo(geometry.Coordinate));
        }

        [Test]
        public void 初期配置は毎回同じになる()
        {
            var first = new TargetGenerator().GenerateTargets(4, TargetType.Compass, new List<TargetGeometry>());
            var second = new TargetGenerator().GenerateTargets(4, TargetType.Compass, new List<TargetGeometry>());

            Assert.That(second.Count, Is.EqualTo(first.Count));
            for (var i = 0; i < first.Count; i++)
            {
                Assert.That(second[i].Coordinate.X, Is.EqualTo(first[i].Coordinate.X).Within(Tolerance));
                Assert.That(second[i].Coordinate.Y, Is.EqualTo(first[i].Coordinate.Y).Within(Tolerance));
                Assert.That(second[i].Size, Is.EqualTo(first[i].Size).Within(Tolerance));
            }
        }

        // ------------------------------------------------------ 当たったあとの移動

        [Test]
        public void 移動先は画面内に収まる()
        {
            var generator = new TargetGenerator();
            var targets = generator.GenerateTargets(4, TargetType.Compass, new List<TargetGeometry>());
            var moved = targets[0];
            var others = new List<TargetGeometry> { targets[1], targets[2], targets[3] };

            for (var i = 0; i < 200; i++)
            {
                var destination = generator.PickRelocation(moved, others);

                Assert.That(destination.X, Is.InRange(moved.Radius, 1920f - moved.Radius));
                Assert.That(destination.Y, Is.InRange(moved.Radius, 1080f - moved.Radius));
            }
        }

        [Test]
        public void 移動先は他の的と重ならない()
        {
            var generator = new TargetGenerator();
            var targets = generator.GenerateTargets(4, TargetType.Compass, new List<TargetGeometry>());
            var moved = targets[0];
            var others = new List<TargetGeometry> { targets[1], targets[2], targets[3] };

            for (var i = 0; i < 200; i++)
            {
                var destination = generator.PickRelocation(moved, others);

                foreach (var other in others)
                {
                    var dx = destination.X - other.Coordinate.X;
                    var dy = destination.Y - other.Coordinate.Y;
                    var distance = (float)System.Math.Sqrt(dx * dx + dy * dy);

                    Assert.That(distance, Is.GreaterThan(moved.Radius + other.Radius),
                        $"移動先が {other} と重なっている");
                }
            }
        }

        [Test]
        public void 移動先は毎回同じにはならない()
        {
            // 決定的に選ぶと、空いた場所が必ず最も条件のよい点になり、
            // 撃った的が同じところへ戻ってきてしまう。
            var generator = new TargetGenerator();
            var targets = generator.GenerateTargets(4, TargetType.Compass, new List<TargetGeometry>());
            var moved = targets[0];
            var others = new List<TargetGeometry> { targets[1], targets[2], targets[3] };

            var first = generator.PickRelocation(moved, others);

            var differs = false;
            for (var i = 0; i < 50 && !differs; i++)
            {
                var next = generator.PickRelocation(moved, others);
                differs = System.Math.Abs(next.X - first.X) > Tolerance
                       || System.Math.Abs(next.Y - first.Y) > Tolerance;
            }

            Assert.That(differs, Is.True, "50 回引いて一度も違う場所が出ないのは乱数が効いていない");
        }

        [Test]
        public void 初期配置は既知の座標になる()
        {
            var targets = new TargetGenerator().GenerateTargets(4, TargetType.Compass, new List<TargetGeometry>());

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
            var targets = new TargetGenerator().GenerateTargets(4, TargetType.Compass, new List<TargetGeometry>());

            for (var i = 0; i < targets.Count; i++)
            {
                for (var j = i + 1; j < targets.Count; j++)
                {
                    var dx = targets[i].Coordinate.X - targets[j].Coordinate.X;
                    var dy = targets[i].Coordinate.Y - targets[j].Coordinate.Y;
                    var distance = Mathf(dx, dy);

                    Assert.That(distance, Is.GreaterThan(targets[i].Radius + targets[j].Radius),
                        $"的 {i} と {j} が重なっている");
                }
            }
        }

        [Test]
        public void 的は画面内に収まる()
        {
            var targets = new TargetGenerator().GenerateTargets(4, TargetType.Compass, new List<TargetGeometry>());

            foreach (var target in targets)
            {
                Assert.That(target.Coordinate.X, Is.InRange(0f, 1920f));
                Assert.That(target.Coordinate.Y, Is.InRange(0f, 1080f));
            }
        }

        private static float Mathf(float dx, float dy) =>
            (float)System.Math.Sqrt(dx * dx + dy * dy);

        private static void AssertTarget(TargetGeometry target, float x, float y, float diameter)
        {
            Assert.That(target.Coordinate.X, Is.EqualTo(x).Within(Tolerance));
            Assert.That(target.Coordinate.Y, Is.EqualTo(y).Within(Tolerance));
            Assert.That(target.Size, Is.EqualTo(diameter).Within(Tolerance));
        }

        [Test]
        public void 盤面の広さを変えると的もその範囲に収まる()
        {
            // 描画側と同じ FieldBounds を渡す。ここが効かないと、
            // 盤面の比率を変えたときに的だけ元の範囲に置かれてしまう。
            var field = new FieldBounds(1280f, 800f);
            var generator = new TargetGenerator(field);

            var targets = generator.GenerateTargets(4, TargetType.Compass, new List<TargetGeometry>());

            foreach (var target in targets)
            {
                Assert.That(target.Coordinate.X, Is.InRange(0f, field.width));
                Assert.That(target.Coordinate.Y, Is.InRange(0f, field.height));
            }
        }

        [Test]
        public void 盤面の広さを変えると移動先もその範囲に収まる()
        {
            var field = new FieldBounds(1280f, 800f);
            var generator = new TargetGenerator(field);

            var targets = generator.GenerateTargets(4, TargetType.Compass, new List<TargetGeometry>());
            var moved = targets[0];

            for (var i = 0; i < 32; i++)
            {
                var destination = generator.PickRelocation(moved, targets);

                Assert.That(destination.X, Is.InRange(moved.Radius, field.width - moved.Radius));
                Assert.That(destination.Y, Is.InRange(moved.Radius, field.height - moved.Radius));
            }
        }
    }
}
