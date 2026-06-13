using System.Collections.Generic;
using Struckout.Domain;


namespace Struckout.Application
{
    public class GameSession
    {
        private readonly ICollisionSolver _collisionSolver;
        private readonly IPointCalculator _pointCalculator;
        private List<Target> _targets;

        public void CollisionDetected()
        {
            
        }
    }
}