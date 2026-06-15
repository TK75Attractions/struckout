using System.Collections.Generic;
using Struckout.Domain;
using Struckout.Dto.V1;


namespace Struckout.Application
{
    public class GameRuntime
    {
        private readonly ICollisionSolver _collisionSolver;
        private readonly IPointCalculator _pointCalculator;
        private readonly ITargetGenerator _targetGenerator;
        
        private readonly GameRuntimeState _state = new();

        public GameRuntime(
            ICollisionSolver collisionSolver,
            IPointCalculator pointCalculator,
            ITargetGenerator targetGenerator
        )
        {
            _collisionSolver = collisionSolver;
            _pointCalculator = pointCalculator;
            _targetGenerator = targetGenerator;
        }

        public void GameSetup()
        {
            _state.AddTargets(_targetGenerator.GenerateTargets(1));
        }

        public void CollisionDetected(CollisionPoint collisionPoint)
        {
            bool isHit = _collisionSolver.IsCollision(collisionPoint, _state.Targets, out Target hitTarget);
            if (!isHit) return;

            _state.AddScore(_pointCalculator.CalculatePoint(hitTarget));
        }
    }
}