using System;
using NUnit.Framework;
using Struckout.Domain;

namespace Struckout.Tests
{
    public class GameClockTests
    {
        [Test]
        public void Hand_completes_one_clockwise_turn_and_stops()
        {
            var clock = new GameClock();
            clock.Start(TimeSpan.FromSeconds(90), 100);
            Assert.That(clock.ClockwiseDegrees(100), Is.Zero);
            Assert.That(clock.ClockwiseDegrees(122.5), Is.EqualTo(90).Within(0.001));
            Assert.That(clock.ClockwiseDegrees(145), Is.EqualTo(180).Within(0.001));
            Assert.That(clock.ClockwiseDegrees(190), Is.EqualTo(360));
            Assert.That(clock.ClockwiseDegrees(200), Is.EqualTo(360));
        }

        [Test]
        public void Correction_extrapolates_between_notifications_and_restart_resets_progress()
        {
            var clock = new GameClock();
            clock.Start(TimeSpan.FromSeconds(90), 100);
            clock.Correct(TimeSpan.FromSeconds(45), 120);
            Assert.That(clock.ClockwiseDegrees(120), Is.EqualTo(180));
            Assert.That(clock.ClockwiseDegrees(120.25), Is.EqualTo(181).Within(0.001));
            clock.Correct(TimeSpan.FromSeconds(-1), 121);
            Assert.That(clock.ClockwiseDegrees(121), Is.EqualTo(360));
            clock.Start(TimeSpan.FromSeconds(60), 200);
            Assert.That(clock.ClockwiseDegrees(200), Is.Zero);
            Assert.That(clock.ClockwiseDegrees(230), Is.EqualTo(180));
        }

        [Test]
        public void Missing_duration_keeps_hand_at_twelve_without_dividing_by_zero()
        {
            var clock = new GameClock();
            clock.Start(TimeSpan.Zero, 0);
            clock.Correct(TimeSpan.FromSeconds(20), 1);
            Assert.That(clock.ClockwiseDegrees(10), Is.Zero);
        }

        [Test]
        public void Added_duration_preserves_elapsed_time_and_recalculates_one_turn()
        {
            var clock = new GameClock();
            clock.Start(TimeSpan.FromSeconds(90), 100);
            clock.Correct(TimeSpan.FromSeconds(70), 120);

            Assert.That(clock.ChangeTotalDuration(TimeSpan.FromSeconds(100), 120),
                Is.EqualTo(TimeSpan.FromSeconds(10)));
            Assert.That(clock.ClockwiseDegrees(120), Is.EqualTo(72).Within(0.001));
            Assert.That(clock.ClockwiseDegrees(160), Is.EqualTo(216).Within(0.001));
            Assert.That(clock.ClockwiseDegrees(200), Is.EqualTo(360));
            Assert.That(clock.ChangeTotalDuration(TimeSpan.FromSeconds(100), 200), Is.EqualTo(TimeSpan.Zero));
        }
    }
}
