using System.Collections.Generic;
using NUnit.Framework;
using Struckout.Application;
using Struckout.Domain;
using Tk75Attractions.Struckout.V1;

namespace Struckout.Tests
{
    /// <summary>
    /// 当たり判定。描画される円と判定の大きさが一致していることを固定する。
    /// 以前は判定半径が描画の 2 倍で、白い円の外側でも当たっていた。
    /// </summary>
    public class CollisionSolverTests
    {
        private static Target Circle(float x, float y, float diameter) =>
            new(new TargetCoordinate(x, y), TargetType.Circle, diameter);

        private static CollisionPoint At(double x, double y) => new() { X = x, Y = y };

        private static CollisionSolver NewSolver() => new(new CollisionCoordinateTransform());

        [Test]
        public void 中心は当たる()
        {
            var targets = new List<Target> { Circle(1000f, 500f, 400f) };

            Assert.That(NewSolver().TryCollision(At(1000, 500), targets, out var hit), Is.True);
            Assert.That(hit.Coordinate.X, Is.EqualTo(1000f));
        }

        /// <summary>直径 400 の的なので、当たる範囲は中心から 200 まで。</summary>
        [TestCase(1199.0, true, TestName = "半径の内側は当たる")]
        [TestCase(1200.0, true, TestName = "半径ちょうどは当たる")]
        [TestCase(1201.0, false, TestName = "半径の外側は外れる")]
        public void 判定は直径の半分までで切り替わる(double x, bool expected)
        {
            var targets = new List<Target> { Circle(1000f, 500f, 400f) };

            Assert.That(NewSolver().TryCollision(At(x, 500), targets, out _), Is.EqualTo(expected));
        }

        [Test]
        public void 斜め方向でも円として判定する()
        {
            // 直径 400 の的。外接する正方形の角は中心から 200*sqrt(2) = 282.8 離れており、
            // 正方形で判定していると当たってしまう。
            var targets = new List<Target> { Circle(1000f, 500f, 400f) };

            Assert.That(NewSolver().TryCollision(At(1200, 700), targets, out _), Is.False,
                "外接正方形の角は円の外なので当たらない");
            Assert.That(NewSolver().TryCollision(At(1141, 641), targets, out _), Is.True,
                "同じ斜め方向でも半径の内側なら当たる");
        }

        [Test]
        public void 的が無ければ必ず外れる()
        {
            Assert.That(NewSolver().TryCollision(At(0, 0), new List<Target>(), out var hit), Is.False);
            Assert.That(hit, Is.Null);
        }

        [Test]
        public void 重なっていない的のうち当たったものを返す()
        {
            var near = Circle(200f, 200f, 100f);
            var far = Circle(1500f, 800f, 100f);
            var targets = new List<Target> { near, far };

            Assert.That(NewSolver().TryCollision(At(1500, 800), targets, out var hit), Is.True);
            Assert.That(hit.Coordinate.X, Is.EqualTo(far.Coordinate.X));
        }
        /// <summary>
        /// 当たった的は座標だけ先に移動先へ移り、画面はそのあと縮んで消え、
        /// 移動先で膨らんで現れる。その間ずっと判定が移動先で有効だと、
        /// 何も描かれていない場所で得点が入る。
        /// </summary>
        [Test]
        public void 移動の演出中は当たらない()
        {
            var target = Circle(1000f, 500f, 400f);
            var targets = new List<Target> { target };

            Assert.That(NewSolver().TryCollision(At(1000, 500), targets, out _), Is.True,
                "前提: 演出前は当たる");

            target.BeginRelocation();

            Assert.That(NewSolver().TryCollision(At(1000, 500), targets, out var hit), Is.False);
            Assert.That(hit, Is.Null);
        }

        [Test]
        public void 演出が終われば再び当たる()
        {
            var target = Circle(1000f, 500f, 400f);
            var targets = new List<Target> { target };

            target.BeginRelocation();
            target.EndRelocation();

            Assert.That(NewSolver().TryCollision(At(1000, 500), targets, out _), Is.True);
        }

        [Test]
        public void 演出中の的は飛ばして他の的を見る()
        {
            // 重ならない 2 つ。手前が演出中でも、奥は普通に当たる。
            var moving = Circle(1000f, 500f, 400f);
            var still = Circle(300f, 500f, 400f);
            moving.BeginRelocation();

            var targets = new List<Target> { moving, still };

            Assert.That(NewSolver().TryCollision(At(300, 500), targets, out var hit), Is.True);
            Assert.That(hit, Is.SameAs(still));
        }

    }
}
