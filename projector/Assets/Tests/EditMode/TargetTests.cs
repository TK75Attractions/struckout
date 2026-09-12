using NUnit.Framework;
using Struckout.Domain;

namespace Struckout.Tests
{
    /// <summary>
    /// Size が直径であることを固定する。
    /// CircleTargetUI は Diameter を sizeDelta に、CollisionSolver は RadiusSquared を
    /// 判定に使うので、この関係が崩れると描画と判定がずれる。
    /// </summary>
    public class TargetTests
    {
        private static Target Circle(float diameter) =>
            new(new TargetCoordinate(0f, 0f), TargetType.Circle, diameter);

        [Test]
        public void Size_はそのまま直径()
        {
            Assert.That(Circle(500f).Diameter, Is.EqualTo(500f));
        }

        [Test]
        public void 半径は直径の半分()
        {
            Assert.That(Circle(500f).Radius, Is.EqualTo(250f));
        }

        [Test]
        public void RadiusSquared_は半径の二乗()
        {
            var target = Circle(500f);

            Assert.That(target.RadiusSquared, Is.EqualTo(target.Radius * target.Radius));
        }

        [Test]
        public void 直径は半径の二倍という関係が保たれる()
        {
            var target = Circle(333f);

            Assert.That(target.Diameter, Is.EqualTo(target.Radius * 2f).Within(0.001f));
        }

        [Test]
        public void 座標が同じでも別の的として扱われる()
        {
            // 的は当たると動く。座標で等価にすると、動かすたびに
            // UIService の辞書との対応づけが壊れてしまう。
            var a = new Target(new TargetCoordinate(10f, 20f), TargetType.Circle, 100f);
            var b = new Target(new TargetCoordinate(10f, 20f), TargetType.Circle, 100f);

            Assert.That(a, Is.Not.EqualTo(b));
            Assert.That(a.Id, Is.Not.EqualTo(b.Id));
        }

        [Test]
        public void 動かしても同じ的のまま()
        {
            var target = new Target(new TargetCoordinate(10f, 20f), TargetType.Circle, 100f);
            int id = target.Id;

            target.MoveTo(new TargetCoordinate(900f, 500f));

            Assert.That(target.Id, Is.EqualTo(id));
            Assert.That(target.Coordinate.X, Is.EqualTo(900f));
            Assert.That(target.Coordinate.Y, Is.EqualTo(500f));
        }

        [Test]
        public void 動かしても大きさは変わらない()
        {
            var target = new Target(new TargetCoordinate(0f, 0f), TargetType.Circle, 400f);

            target.MoveTo(new TargetCoordinate(100f, 100f));

            Assert.That(target.Diameter, Is.EqualTo(400f));
            Assert.That(target.Radius, Is.EqualTo(200f));
        }
        [Test]
        public void 既定では当たり判定の対象()
        {
            Assert.That(new Target(new TargetCoordinate(0f, 0f), TargetType.Circle, 100f).IsHittable,
                Is.True);
        }

        [Test]
        public void 移動の演出中だけ当たり判定から外れる()
        {
            var target = new Target(new TargetCoordinate(0f, 0f), TargetType.Circle, 100f);

            target.BeginRelocation();
            Assert.That(target.IsHittable, Is.False);

            target.EndRelocation();
            Assert.That(target.IsHittable, Is.True);
        }

        [Test]
        public void 演出の状態は大きさと座標に影響しない()
        {
            var target = new Target(new TargetCoordinate(10f, 20f), TargetType.Circle, 100f);

            target.BeginRelocation();

            Assert.That(target.Size, Is.EqualTo(100f));
            Assert.That(target.Radius, Is.EqualTo(50f));
            Assert.That(target.Coordinate.X, Is.EqualTo(10f));
            Assert.That(target.Coordinate.Y, Is.EqualTo(20f));
        }

    }
}
