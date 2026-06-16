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
        private readonly IUIService _uiService;
        private Action<Target> _CollisionTargetAction;
        
        private readonly GameRuntimeState _state = new();

        public GameRuntime(
            ICollisionSolver collisionSolver,
            IPointCalculator pointCalculator,
            ITargetGenerator targetGenerator,
            IUIService uiService
        )
        {
            _collisionSolver = collisionSolver;
            _pointCalculator = pointCalculator;
            _targetGenerator = targetGenerator;
            _uiService = uiService;
        }

        public void GameSetup()
        {
            _state.AddTargets(_targetGenerator.GenerateTargets(1));
            UpdateUI();
        }

        public void AddCollisionTargetAction(Action<Target> action)
        {
            _CollisionTargetAction += action;
        }

        public void RemoveCollisionTargetAction(Action<Target> action)
        {
            _CollisionTargetAction -= action;
        }

        public void UpdateUI()
        {
            _uiService.InstantinateTargets(_state.Targets);
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