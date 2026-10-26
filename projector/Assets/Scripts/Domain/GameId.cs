using System;
using System.Globalization;

namespace Struckout.Domain
{
    public readonly struct GameId : IEquatable<GameId>
    {
        public uint Value { get; }
        public GameId(uint value) => Value = value;
        public bool Equals(GameId other) => Value == other.Value;
        public override bool Equals(object obj) => obj is GameId other && Equals(other);
        public override int GetHashCode() => Value.GetHashCode();
        public override string ToString() => Value.ToString(CultureInfo.InvariantCulture);
    }
}
