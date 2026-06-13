using Struckout.Domain;
using System.Collections.Generic;

namespace Struckout.Application
{
    public class GameRuntimeState
    {
        public List<Target> Targets { get; private set; } = new();
        public int Score { get; set; } = 0;

        
    }
}