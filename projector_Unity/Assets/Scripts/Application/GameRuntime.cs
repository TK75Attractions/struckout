using System;
using Struckout.Domain;
using Struckout.Dto.V1;


namespace Struckout.Application
{
    public class GameRuntime
    {
        private readonly ICollisionSolver _collisionSolver;
        private readonly IPointCalculator _pointCalculator;
        private readonly ITargetGenerator _targetGenerator;
        private Action<Target> _CollisionTargetAction;
        
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

        public void AddCollisionTargetAction(Action<Target> action)
        {
            _CollisionTargetAction += action;
        }

        public void RemoveCollisionTargetAction(Action<Target> action)
        {
            _CollisionTargetAction -= action;
        }

        public void CollisionDetected(CollisionPoint collisionPoint)
        {
            if(_collisionSolver.TryGetCollision(collisionPoint,_state.Targets,out Target hitTarget))
            {
                _CollisionTargetAction?.Invoke(hitTarget);
                _state.AddScore(_pointCalculator.CalculatePoint(hitTarget));
            }
        }
    }
}