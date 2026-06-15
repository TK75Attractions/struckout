using Struckout.Domain;
using System.Collections.Generic;
using System;

namespace Struckout.Application
{
    public class GameRuntimeState
    {
        private List<Target> _targets = new();
        public IReadOnlyList<Target> Targets => _targets;
        public int Score { get; set; } = 0;

        public void AddTargets(List<Target> targets)
        {
            foreach (var target in targets)
            {
                AddTarget(target);
            }   
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