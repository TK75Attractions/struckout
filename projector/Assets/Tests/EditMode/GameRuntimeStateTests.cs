using NUnit.Framework;
using Struckout.Application;
using Struckout.Domain;
using Tk75Attractions.Struckout.V1;

namespace Struckout.Tests
{
    public class GameRuntimeStateTests
    {
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
