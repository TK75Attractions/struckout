using Struckout.Domain;
using System.Collections.Generic;
using System;

namespace Struckout.Application
{
    public class GameRuntimeState
    {
        public List<Target> Targets { get; private set; } = new();
        public int Score { get; set; } = 0;

        public void AddTarget(Target target)
        {
            if (Targets.Contains(target)) throw new Exception("Add Existing Target");
            Targets.Add(target);
        }

        public void RemoveTarget(Target target)
        {
            if (!Targets.Contains(target)) throw new Exception("Remove Missing Target");
            Targets.Remove(target);
        }
    }
}