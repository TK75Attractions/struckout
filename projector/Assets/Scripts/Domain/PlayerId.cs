using System;
using System.Globalization;

namespace Struckout.Domain
{
    public readonly struct PlayerId : IEquatable<PlayerId>
    {
        public uint Value { get; }
        public PlayerId(uint value) => Value = value;
        public bool Equals(PlayerId other) => Value == other.Value;
        public override bool Equals(object obj) => obj is PlayerId other && Equals(other);
        public override int GetHashCode() => Value.GetHashCode();
        public override string ToString() => Value.ToString(CultureInfo.InvariantCulture);
    }
}
