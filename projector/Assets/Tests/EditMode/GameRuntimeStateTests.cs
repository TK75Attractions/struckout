using NUnit.Framework;
using Struckout.Application;
using Struckout.Domain;
using Tk75Attractions.Struckout.GameMaster.V1;

namespace Struckout.Tests.Tests.EditMode
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

        [Test]
        public void 終了後は_Finished()
        {
            var state = new GameRuntimeState();
            state.StartGame(Difficulty.Normal);
            state.FinishGame();
            Assert.That(state.Phase, Is.EqualTo(GamePhase.Finished));
        }

        [Test]
        public void 切断時は_Idle()
        {
            var state = new GameRuntimeState();
            state.StartGame(Difficulty.Normal);
            state.SuspendGame();
            Assert.That(state.Phase, Is.EqualTo(GamePhase.Idle));
        }
    }
}
