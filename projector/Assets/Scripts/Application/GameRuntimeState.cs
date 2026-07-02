using Struckout.Domain;
using System.Collections.Generic;
using System;
using Tk75Attractions.Struckout.V1;

namespace Struckout.Application
{
    public class GameRuntimeState
    {
        private List<Target> _targets = new();
        public IReadOnlyList<Target> Targets => _targets;
        public int Score { get; private set; } = 0;
        public Difficulty Difficulty { get; private set;}

        public void SetDifficulty(Difficulty difficulty)
        {
            Difficulty = difficulty;
        }
        public void AddTargets(IReadOnlyList<Target> targets)
        {
            foreach (var target in targets)
            {
                AddTarget(target);
            }   
        }

        public void AddScore(int score)
        {
            Score += score;
        }

        public void DecreaseScore(int score)
        {
            if (score > Score)
            {
                Score = 0;
                return;
            }
            Score -= score;
        }

        public void AddTarget(Target target)
        {
            if (_targets.Contains(target)) throw new Exception("Add Existing Target");
            _targets.Add(target);
        }

        public void RemoveTarget(Target target)
        {
            if (!_targets.Contains(target)) throw new Exception("Remove Missing Target");
            _targets.Remove(target);
        }
    }
}