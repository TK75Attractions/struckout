using Struckout.Domain;
using System.Collections.Generic;
using System;
using Struckout.Domain.TargetDefinition;
using Difficulty = Tk75Attractions.Struckout.GameMaster.V1.Difficulty;

namespace Struckout.Application
{
    internal class GameRuntimeState
    {
        private readonly List<Target> _targets = new();
        public IReadOnlyList<Target> Targets => _targets;

        /// <summary>StartGame を受け取るまでは Idle。</summary>
        public GamePhase Phase { get; private set; } = GamePhase.Idle;

        public void StartGame(Difficulty difficulty)
        {
            SetDifficulty(difficulty);
            Phase = GamePhase.Playing;
        }

        /// <summary>
        /// 制限時間が尽きた。game_master から GameFinished が届いたときに呼ぶ。
        /// 以降は当たっても得点にしない。
        /// </summary>
        public void FinishGame()
        {
            Phase = GamePhase.Finished;
        }

        public void SuspendGame() => Phase = GamePhase.Idle;
        public Difficulty Difficulty { get; private set; }

        public void SetDifficulty(Difficulty difficulty)
        {
            Difficulty = difficulty;
        }
        
        public void AddTarget(Target target)
        {
            if (_targets.Exists(existing => existing.Id == target.Id))
                throw new InvalidOperationException($"Target {target.Id} is already registered.");
            _targets.Add(target);
        }

        public void RemoveTarget(Guid id)
        {
            var index = _targets.FindIndex(target => target.Id == id);
            if (index < 0) throw new InvalidOperationException($"Target {id} is not registered.");
            _targets.RemoveAt(index);
        }
    }
}
