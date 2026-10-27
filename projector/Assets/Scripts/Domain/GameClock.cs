using System;

namespace Struckout.Domain
{
    /// <summary>
    /// Extrapolates server time using a monotonic clock so every watch shares game progress,
    /// including watches added after the game has started.
    /// </summary>
    public sealed class GameClock
    {
        private double _durationSeconds;
        private double _remainingSeconds;
        private double _notifiedAt;

        public void Start(TimeSpan duration, double now)
        {
            _durationSeconds = Math.Max(0, duration.TotalSeconds);
            _remainingSeconds = _durationSeconds;
            _notifiedAt = now;
        }

        public void Correct(TimeSpan remaining, double now)
        {
            _remainingSeconds = Math.Clamp(remaining.TotalSeconds, 0, _durationSeconds);
            _notifiedAt = now;
        }

        public double ClockwiseDegrees(double now)
        {
            if (_durationSeconds <= 0) return 0;
            var remaining = Math.Clamp(_remainingSeconds - Math.Max(0, now - _notifiedAt), 0, _durationSeconds);
            return 360 * (1 - remaining / _durationSeconds);
        }
    }
}
