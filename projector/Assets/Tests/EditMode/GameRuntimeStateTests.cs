using NUnit.Framework;
using Struckout.Application;
using Struckout.Domain;
using Tk75Attractions.Struckout.V1;

namespace Struckout.Tests
{
    public class GameRuntimeStateTests
    {
        private static Target Circle(float x) =>
            new(new TargetCoordinate(x, 0f), TargetType.Circle, 100f);

        /// <summary>時間を任意に進められる時計。</summary>
        private sealed class FakeClock
        {
            public long NowMs;
            public long Read() => NowMs;
            public void Advance(long ms) => NowMs += ms;
        }

        // ---------------------------------------------------------------- phase

        [Test]
        public void 開始前は_Idle()
        {
            Assert.That(new GameRuntimeState().Phase, Is.EqualTo(GamePhase.Idle));
        }

        [Test]
        public void StartGame_で_Playing_になり難易度が入る()
        {
            var state = new GameRuntimeState();

            state.StartGame(Difficulty.Hard);

            Assert.That(state.Phase, Is.EqualTo(GamePhase.Playing));
            Assert.That(state.Difficulty, Is.EqualTo(Difficulty.Hard));
        }

        // ---------------------------------------------------------------- cooldown

        [Test]
        public void 撃たれていない的はクールダウンしていない()
        {
            var state = new GameRuntimeState(new FakeClock().Read);

            Assert.That(state.IsCoolingDown(Circle(0f)), Is.False);
        }

        [Test]
        public void クールダウン中は当たらない扱いになる()
        {
            var clock = new FakeClock();
            var state = new GameRuntimeState(clock.Read);
            var target = Circle(0f);

            state.StartCooldown(target, 1.5f);

            Assert.That(state.IsCoolingDown(target), Is.True);
        }

        [Test]
        public void 時間が経てばクールダウンは解ける()
        {
            var clock = new FakeClock();
            var state = new GameRuntimeState(clock.Read);
            var target = Circle(0f);

            state.StartCooldown(target, 1.5f);

            clock.Advance(1499);
            Assert.That(state.IsCoolingDown(target), Is.True, "解除の直前はまだクールダウン中");

            clock.Advance(1);
            Assert.That(state.IsCoolingDown(target), Is.False, "指定した秒数でちょうど解ける");
        }

        [Test]
        public void クールダウンは的ごとに独立している()
        {
            var clock = new FakeClock();
            var state = new GameRuntimeState(clock.Read);
            var shot = Circle(0f);
            var untouched = Circle(500f);

            state.StartCooldown(shot, 1.5f);

            Assert.That(state.IsCoolingDown(shot), Is.True);
            Assert.That(state.IsCoolingDown(untouched), Is.False);
        }

        [Test]
        public void 秒数が0以下ならクールダウンしない()
        {
            var clock = new FakeClock();
            var state = new GameRuntimeState(clock.Read);
            var target = Circle(0f);

            state.StartCooldown(target, 0f);

            Assert.That(state.IsCoolingDown(target), Is.False);
        }

        [Test]
        public void 撃ち直すとクールダウンが延長される()
        {
            var clock = new FakeClock();
            var state = new GameRuntimeState(clock.Read);
            var target = Circle(0f);

            state.StartCooldown(target, 1.0f);
            clock.Advance(900);
            state.StartCooldown(target, 1.0f);

            clock.Advance(200);
            Assert.That(state.IsCoolingDown(target), Is.True, "撃ち直した時点から数え直す");
        }

        // ---------------------------------------------------------------- score

        [Test]
        public void 得点は加算される()
        {
            var state = new GameRuntimeState();

            state.AddScore(3);
            state.AddScore(4);

            Assert.That(state.Score, Is.EqualTo(7));
        }

        [Test]
        public void 得点は0を下回らない()
        {
            var state = new GameRuntimeState();

            state.AddScore(2);
            state.DecreaseScore(5);

            Assert.That(state.Score, Is.EqualTo(0));
        }
    }
}
