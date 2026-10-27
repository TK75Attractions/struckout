using System;
using System.Collections.Generic;
using NUnit.Framework;
using Struckout.Domain;
using Struckout.Domain.TargetDefinition;
using Struckout.Infrastructure;
using Tk75Attractions.Struckout.V1;

namespace Struckout.Tests.Tests.EditMode
{
    /// <summary>
    /// 当たり判定。描画される円と判定の大きさが一致していることを固定する。
    /// 以前は判定半径が描画の 2 倍で、白い円の外側でも当たっていた。
    /// </summary>
    public class CollisionSolverTests
    {
        private static Target Compass(float x, float y, float diameter) =>
            new(Guid.NewGuid(), new TargetGeometry(new TargetCoordinate(x, y), TargetType.Compass, diameter), TargetOwnership.Single);

        private static CollisionPoint At(double x, double y) => new() { X = x, Y = y };

        private static CollisionSolver NewSolver() => new(new CollisionCoordinateTransform());

        [Test]
        public void 中心は当たる()
        {
            var targets = new List<Target> { Compass(1000f, 500f, 400f) };

            Assert.That(NewSolver().TryCollision(At(1000, 500), targets, out var hit), Is.True);
            Assert.That(hit.Coordinate.X, Is.EqualTo(1000f));
        }

        /// <summary>直径 400 の的なので、当たる範囲は中心から 200 まで。</summary>
        [TestCase(1199.0, true, TestName = "半径の内側は当たる")]
        [TestCase(1200.0, true, TestName = "半径ちょうどは当たる")]
        [TestCase(1201.0, false, TestName = "半径の外側は外れる")]
        public void 判定は直径の半分までで切り替わる(double x, bool expected)
        {
            var targets = new List<Target> { Compass(1000f, 500f, 400f) };

            Assert.That(NewSolver().TryCollision(At(x, 500), targets, out _), Is.EqualTo(expected));
        }

        [Test]
        public void 斜め方向でも円として判定する()
        {
            // 直径 400 の的。外接する正方形の角は中心から 200*sqrt(2) = 282.8 離れており、
            // 正方形で判定していると当たってしまう。
            var targets = new List<Target> { Compass(1000f, 500f, 400f) };

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
            var near = Compass(200f, 200f, 100f);
            var far = Compass(1500f, 800f, 100f);
            var targets = new List<Target> { near, far };

            Assert.That(NewSolver().TryCollision(At(1500, 800), targets, out var hit), Is.True);
            Assert.That(hit.Coordinate.X, Is.EqualTo(far.Coordinate.X));
        }
        [TestCase(1099, 500, true)]
        [TestCase(1100, 500, true)]
        [TestCase(1101, 500, false)]
        [TestCase(1100, 600, false)]
        public void Pocket_watch_hit_area_matches_the_dial(double x, double y, bool expected)
        {
            var watch = new Target(Guid.NewGuid(),
                new TargetGeometry(new TargetCoordinate(1000, 500), TargetType.PocketWatch, 200), TargetOwnership.All);
            Assert.That(NewSolver().TryCollision(At(x, y), new[] { watch }, out _), Is.EqualTo(expected));
        }

    }
}
