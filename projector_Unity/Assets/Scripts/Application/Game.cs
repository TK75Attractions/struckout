using System.Collections.Generic;
using Struckout.Domain;
using Struckout.Dto.V1;


namespace Struckout.Application
{
    public class Game
    {
        private readonly ICollisionSolver _collisionSolver;
        private readonly IPointCalculator _pointCalculator;
        
        private GameRuntimeState _state = new();

        public void CollisionDetected(CollisionPoint collisionPoint)
        {
            bool isHit = _collisionSolver.IsCollision(collisionPoint, _state.Targets, out Target hittedTarget);
            if (!isHit) return;

            _state.Score += _pointCalculator.CalculatePoint(hittedTarget);
        }
    }
}