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
        public void 同じ値の的は等しいものとして扱われる()
        {
            // UIService は Target を辞書の鍵にしているので、値等価が要る。
            var a = new Target(new TargetCoordinate(10f, 20f), TargetType.Circle, 100f);
            var b = new Target(new TargetCoordinate(10f, 20f), TargetType.Circle, 100f);

            Assert.That(a, Is.EqualTo(b));
            Assert.That(a.GetHashCode(), Is.EqualTo(b.GetHashCode()));
        }
    }
}
