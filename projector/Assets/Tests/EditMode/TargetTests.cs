using System;
using NUnit.Framework;
using Struckout.Domain.TargetDefinition;
using Struckout.Domain;

namespace Struckout.Tests.Tests.EditMode
{
    public class TargetTests
    {
        private static Target Compass(float diameter) =>
            new(Guid.NewGuid(), new TargetGeometry(new TargetCoordinate(0f, 0f), TargetType.Compass, diameter), TargetOwnership.Single);

        [Test]
        public void Size_はそのまま直径() => Assert.That(Compass(500f).Diameter, Is.EqualTo(500f));

        [Test]
        public void 半径は直径の半分() => Assert.That(Compass(500f).Radius, Is.EqualTo(250f));

        [Test]
        public void RadiusSquared_は半径の二乗()
        {
            var target = Compass(500f);
            Assert.That(target.RadiusSquared, Is.EqualTo(target.Radius * target.Radius));
        }

        [Test]
        public void 直径は半径の二倍という関係が保たれる()
        {
            var target = Compass(333f);
            Assert.That(target.Diameter, Is.EqualTo(target.Radius * 2f).Within(0.001f));
        }

        [Test]
        public void Coordinates_do_not_define_entity_identity()
        {
            var geometry = new TargetGeometry(new TargetCoordinate(10f, 20f), TargetType.Compass, 100f);
            var first = new Target(Guid.NewGuid(), geometry, TargetOwnership.Single);
            var second = new Target(Guid.NewGuid(), geometry, TargetOwnership.Single);
            Assert.That(first, Is.Not.EqualTo(second));
        }

        [Test]
        public void Uuid_defines_identity_even_for_separately_constructed_instances()
        {
            var id = Guid.NewGuid();
            var geometry = new TargetGeometry(new TargetCoordinate(10f, 20f), TargetType.Compass, 100f);
            var first = new Target(id, geometry, TargetOwnership.Single);
            var second = new Target(id, geometry, TargetOwnership.Single);
            Assert.That(first.Id, Is.EqualTo(id));
            Assert.That(second, Is.EqualTo(first));
            Assert.That(second.GetHashCode(), Is.EqualTo(first.GetHashCode()));
        }

        [Test]
        public void Replacement_is_a_different_entity_with_immutable_original_geometry()
        {
            var original = Compass(400f);
            var replacement = new Target(Guid.NewGuid(),
                new TargetGeometry(new TargetCoordinate(100f, 100f), original.Type, original.Size), TargetOwnership.Single);
            Assert.That(replacement, Is.Not.EqualTo(original));
            Assert.That(original.Coordinate.X, Is.Zero);
            Assert.That(original.Coordinate.Y, Is.Zero);
            Assert.That(replacement.Diameter, Is.EqualTo(original.Diameter));
        }

        [Test]
        public void Empty_uuid_cannot_construct_an_unregistered_target()
        {
            Assert.Throws<ArgumentException>(() => new Target(Guid.Empty,
                new TargetGeometry(default, TargetType.Compass, 100f), TargetOwnership.Single));
        }

        [Test]
        public void Moving_target_reflects_at_both_field_edges_and_keeps_registered_geometry()
        {
            var geometry = new TargetGeometry(new TargetCoordinate(70, 70), TargetType.Compass, 20,
                new TargetVelocity(100, -100));
            var target = new Target(Guid.NewGuid(), geometry, TargetOwnership.Single);
            var field = new FieldBounds(100, 100);

            target.Advance(0.5, field);
            Assert.That(target.Coordinate.X, Is.EqualTo(60).Within(0.001));
            Assert.That(target.Coordinate.Y, Is.EqualTo(20).Within(0.001));
            target.Advance(2, field);
            Assert.That(target.Coordinate.X, Is.EqualTo(20).Within(0.001));
            Assert.That(target.Coordinate.Y, Is.EqualTo(40).Within(0.001));
            Assert.That(target.Geometry.Coordinate.X, Is.EqualTo(70));
            Assert.That(target.Geometry.Coordinate.Y, Is.EqualTo(70));
        }
    }
}
