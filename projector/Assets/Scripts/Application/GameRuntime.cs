using System;
using Struckout.Domain;
using Tk75Attractions.Struckout.V1;


namespace Struckout.Application
{
    public class GameRuntime
    {
        private readonly ICollisionSolver _collisionSolver;
        private readonly IPointCalculator _pointCalculator;
        private readonly ITargetGenerator _targetGenerator;
        private readonly IUIService _uiService;
        private Action<Target> _collisionTargetAction;
        
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
            _state.AddTargets(_targetGenerator.GenerateTargets(4, 8));
            AddCollisionTargetAction(_state.RemoveTarget);
            UpdateUI();
        }

        public void AddCollisionTargetAction(Action<Target> action)
        {
            _collisionTargetAction += action;
        }

        public void RemoveCollisionTargetAction(Action<Target> action)
        {
            _collisionTargetAction -= action;
        }

        public void UpdateUI()
        {
            _uiService.InstantiateTargets(_state.Targets);
        }

        public void CollisionDetected(CollisionPoint collisionPoint)
        {
            if(_collisionSolver.TryCollision(collisionPoint, _state.Targets, out Target hitTarget))
            {
                _collisionTargetAction?.Invoke(hitTarget);
                _state.AddScore(_pointCalculator.CalculatePoint(hitTarget));
            }
        }
    }
}