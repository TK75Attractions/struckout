using System;
using Struckout.Domain;
using Tk75Attractions.Struckout.V1;
using UnityEngine;


namespace Struckout.Application
{
    public class GameRuntime
    {
        private readonly ICollisionSolver _collisionSolver;
        private readonly IPointCalculator _pointCalculator;
        private readonly ITargetGenerator _targetGenerator;
        private readonly IUIService _uiService;
        private readonly GameSettings _settings;
        private Action<Target> _collisionTargetAction;

        /// <summary>
        /// 的に当たって得点が入ったときの「増分」。
        /// game_master (session.rs) は cur_score += score としているので、累計ではなく差分を流す。
        /// </summary>
        public event Action<int> ScoreAdded;
        
        private readonly GameRuntimeState _state = new();

        public GameRuntime(
            ICollisionSolver collisionSolver,
            IPointCalculator pointCalculator,
            ITargetGenerator targetGenerator,
            IUIService uiService,
            GameSettings settings
        )
        {
            _collisionSolver = collisionSolver;
            _pointCalculator = pointCalculator;
            _targetGenerator = targetGenerator;
            _uiService = uiService;
            _settings = settings ?? throw new ArgumentNullException(nameof(settings));
        }

        public void GameSetup()
        {
            // 的は最初に置いたきり、位置も個数も変えない。
            // 撃たれたときは消さずにクールダウンに入る。
            _state.AddTargets(_targetGenerator, _settings.InitialTargetCount, TargetType.Circle);
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

        /// <summary>
        /// UI がまだ無い的の分だけ生成する。
        /// 実装側が生成済みの的を読み飛ばすので、何度呼んでも二重には作られない。
        /// </summary>
        public void UpdateUI()
        {
            _uiService.InstantiateTargets(_state.Targets);
        }

        public void CollisionDetected(CollisionPoint collisionPoint)
        {
            bool hit = _collisionSolver.TryCollision(collisionPoint, _state.Targets, out Target hitTarget);

            // 着弾位置は当たり外れに関わらず出す。外れが見えないと、
            // 座標変換がずれているのか単に的を外したのかが区別できない。
            // また「外した」と「クールダウン中の的に当たった」は見た目で区別できないと紛らわしいので、
            // 結果を 3 状態に分けてマーカーの色を変える。
            CollisionResult result;
            if (!hit) result = CollisionResult.Missed;
            else if (_state.IsCoolingDown(hitTarget)) result = CollisionResult.CoolingDown;
            else result = CollisionResult.Scored;

            _uiService.ShowCollisionMarker((float)collisionPoint.X, (float)collisionPoint.Y, result);

            if (result != CollisionResult.Scored)
            {
                Debug.Log(result == CollisionResult.CoolingDown
                    ? $"[Hit] cooling down: target at ({hitTarget.Coordinate.X:F0}, {hitTarget.Coordinate.Y:F0})"
                    : $"[Hit] missed: point=({collisionPoint.X:F0}, {collisionPoint.Y:F0}) targets={_state.Targets.Count}");
                return;
            }

            _collisionTargetAction?.Invoke(hitTarget);

            int points = _pointCalculator.CalculatePoint(hitTarget);
            _state.AddScore(points);
            ScoreAdded?.Invoke(points);

            _state.StartCooldown(hitTarget, _settings.TargetCooldownSeconds);
            _uiService.OnTargetHit(hitTarget, _settings.TargetCooldownSeconds);
        }
    }
}