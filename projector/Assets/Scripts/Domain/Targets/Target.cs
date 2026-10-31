using System;
using Struckout.Domain;

namespace Struckout.Domain.TargetDefinition
{
    /// <summary>
    /// A registered target received in game_master's TargetAdded event.
    /// Replacement creates a new entity; registered geometry and UUID never change.
    /// The current coordinate advances locally so rendering and hit detection share it.
    /// </summary>
    public sealed class Target : IEquatable<Target>
    {
        public Guid Id { get; }
        public TargetGeometry Geometry { get; }
        public TargetOwnership Ownership { get; }
        public TargetCoordinate Coordinate { get; private set; }
        public TargetType Type => Geometry.Type;
        public float Size => Geometry.Size;
        public float Diameter => Geometry.Diameter;
        public float Radius => Geometry.Radius;
        public float RadiusSquared => Geometry.RadiusSquared;
        private double _elapsedSeconds;

        public Target(Guid id, TargetGeometry geometry, TargetOwnership ownership)
        {
            if (id == Guid.Empty) throw new ArgumentException("A registered target requires a non-empty UUID.", nameof(id));
            Id = id;
            Geometry = geometry;
            Ownership = ownership;
            Coordinate = geometry.Coordinate;
        }

        /// <summary>Advance from the registered position, reflecting at edges without losing time on long frames.</summary>
        public void Advance(double deltaSeconds, FieldBounds field)
        {
            if (deltaSeconds <= 0 || (Geometry.Velocity.X == 0 && Geometry.Velocity.Y == 0)) return;
            _elapsedSeconds += deltaSeconds;
            Coordinate = new TargetCoordinate(
                Reflect(Geometry.Coordinate.X, Geometry.Velocity.X, Radius, field.width - Radius, _elapsedSeconds),
                Reflect(Geometry.Coordinate.Y, Geometry.Velocity.Y, Radius, field.height - Radius, _elapsedSeconds));
        }

        private static float Reflect(double start, double velocity, double min, double max, double seconds)
        {
            if (max <= min) return (float)((min + max) / 2);
            var length = max - min;
            var position = (start - min + velocity * seconds) % (2 * length);
            if (position < 0) position += 2 * length;
            return (float)(min + (position <= length ? position : 2 * length - position));
        }

        public bool Equals(Target other) => other != null && Id == other.Id;
        public override bool Equals(object obj) => obj is Target other && Equals(other);
        public override int GetHashCode() => Id.GetHashCode();

        public override string ToString() =>
            $"Target#{Id} ({Coordinate.X:F1}, {Coordinate.Y:F1}) d={Size:F1}";
    }
}
