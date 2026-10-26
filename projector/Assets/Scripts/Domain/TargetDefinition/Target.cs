using System;

namespace Struckout.Domain.TargetDefinition
{
    /// <summary>
    /// A registered target received in game_master's TargetAdded event.
    /// Replacement creates a new entity; existing geometry and UUID never change.
    /// </summary>
    public sealed class Target : IEquatable<Target>
    {
        public Guid Id { get; }
        public TargetGeometry Geometry { get; }
        public TargetOwnership Ownership { get; }
        public TargetCoordinate Coordinate => Geometry.Coordinate;
        public TargetType Type => Geometry.Type;
        public float Size => Geometry.Size;
        public float Diameter => Geometry.Diameter;
        public float Radius => Geometry.Radius;
        public float RadiusSquared => Geometry.RadiusSquared;

        public Target(Guid id, TargetGeometry geometry, TargetOwnership ownership)
        {
            if (id == Guid.Empty) throw new ArgumentException("A registered target requires a non-empty UUID.", nameof(id));
            Id = id;
            Geometry = geometry;
            Ownership = ownership;
        }

        public bool Equals(Target other) => other != null && Id == other.Id;
        public override bool Equals(object obj) => obj is Target other && Equals(other);
        public override int GetHashCode() => Id.GetHashCode();

        public override string ToString() =>
            $"Target#{Id} ({Coordinate.X:F1}, {Coordinate.Y:F1}) d={Size:F1}";
    }
}
