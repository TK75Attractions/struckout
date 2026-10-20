using System;
using NUnit.Framework;
using Struckout.Domain;

namespace Struckout.Tests.Tests.EditMode
{
    /// <summary>
    /// 実行中の調整画面 (F2) が使う、2 点からの係数の計算と、逆変換・値の写し。
    /// </summary>
    public class CollisionTransformFitTests
    {
        private const double Tolerance = 0.01;

        private static readonly (double X, double Y) Render1 = (480, 270);
        private static readonly (double X, double Y) Render2 = (1440, 810);

        [Test]
        public void 二点から係数を決めると_その二点に写る()
        {
            var transform = new CollisionCoordinateTransform();

            transform.FitTwoPoints(Render1, (0.10, 1.80), Render2, (1.10, 2.40));

            AssertMapsTo(transform, (0.10, 1.80), Render1);
            AssertMapsTo(transform, (1.10, 2.40), Render2);
            Assert.That(transform.pixelsPerMetreX, Is.EqualTo(960.0).Within(Tolerance));
            Assert.That(transform.pixelsPerMetreY, Is.EqualTo(900.0).Within(Tolerance));
            Assert.That(transform.flipX, Is.False);
            Assert.That(transform.flipY, Is.False);
        }

        [Test]
        public void 向きが逆なら_Flip_を立てて_1m_あたりは正のまま()
        {
            var transform = new CollisionCoordinateTransform();

            // 盤面の左下の点が、実際には右上に映っていた
            transform.FitTwoPoints(Render1, (1.10, 2.40), Render2, (0.10, 1.80));

            Assert.That(transform.flipX, Is.True);
            Assert.That(transform.flipY, Is.True);
            Assert.That(transform.pixelsPerMetreX, Is.GreaterThan(0f));
            Assert.That(transform.pixelsPerMetreY, Is.GreaterThan(0f));
            AssertMapsTo(transform, (1.10, 2.40), Render1);
            AssertMapsTo(transform, (0.10, 1.80), Render2);
        }

        [Test]
        public void SwapAxes_のままでも二点に写る()
        {
            var transform = new CollisionCoordinateTransform { swapAxes = true };

            transform.FitTwoPoints(Render1, (0.10, 1.80), Render2, (1.10, 2.40));

            Assert.That(transform.swapAxes, Is.True);
            AssertMapsTo(transform, (0.10, 1.80), Render1);
            AssertMapsTo(transform, (1.10, 2.40), Render2);
        }

        [Test]
        public void 二点が同じ高さだと計算できず係数も変えない()
        {
            var transform = new CollisionCoordinateTransform();
            var before = transform.ToString();

            Assert.Throws<ArgumentException>(() =>
                transform.FitTwoPoints(Render1, (0.10, 2.0), Render2, (1.10, 2.0)));
            Assert.That(transform.ToString(), Is.EqualTo(before));
        }

        [TestCase(false, false, false)]
        [TestCase(true, false, false)]
        [TestCase(false, true, true)]
        [TestCase(true, true, true)]
        public void ToPhysical_は_ToRenderSpace_の逆(bool flipX, bool flipY, bool swap)
        {
            var transform = new CollisionCoordinateTransform
            {
                pixelsPerMetreX = 870f,
                pixelsPerMetreY = 910f,
                originX = 120f,
                originY = -1500f,
                flipX = flipX,
                flipY = flipY,
                swapAxes = swap,
            };

            var (rx, ry) = transform.ToRenderSpace(0.37, 2.21);
            var (x, y) = transform.ToPhysical(rx, ry);

            Assert.That(x, Is.EqualTo(0.37).Within(1e-6));
            Assert.That(y, Is.EqualTo(2.21).Within(1e-6));
        }

        [Test]
        public void CopyFrom_はすべての値を写す()
        {
            var source = new CollisionCoordinateTransform
            {
                pixelsPerMetreX = 1f,
                pixelsPerMetreY = 2f,
                originX = 3f,
                originY = 4f,
                flipX = true,
                flipY = true,
                swapAxes = true,
                logConversions = true,
            };
            var target = new CollisionCoordinateTransform();

            target.CopyFrom(source);

            Assert.That(target.ToString(), Is.EqualTo(source.ToString()));
            Assert.That(target.logConversions, Is.True);
        }

        private static void AssertMapsTo(
            CollisionCoordinateTransform transform, (double X, double Y) physical, (double X, double Y) expected)
        {
            var (x, y) = transform.ToRenderSpace(physical.X, physical.Y);
            Assert.That(x, Is.EqualTo(expected.X).Within(Tolerance));
            Assert.That(y, Is.EqualTo(expected.Y).Within(Tolerance));
        }
    }
}
