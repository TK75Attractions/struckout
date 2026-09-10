using System;
using System.Collections.Generic;
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
            // 個数は変えない。撃たれた的は消えず、別の場所へ移動する。
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

        /// <summary>
        /// game_master から StartGame が届いたときに呼ぶ。これを受けるまでは得点を送らない。
        /// </summary>
        public void StartGame(Difficulty difficulty)
        {
            _state.StartGame(difficulty);
            Debug.Log($"[Game] started ({difficulty})");
        }

        /// <summary>
        /// game_master から GameFinished が届いたときに呼ぶ。以降は得点を送らない。
        /// </summary>
        public void FinishGame()
        {
            _state.FinishGame();
            Debug.Log($"[Game] finished (score={_state.Score})");
        }

        public void CollisionDetected(CollisionPoint collisionPoint)
        {
            float x = (float)collisionPoint.X;
            float y = (float)collisionPoint.Y;

            // ゲームが動いていない間に当たっても判定しない。
            // game_master はセッション外の得点を running_games に見つけられず、
            // NotFound を返す (service.rs の add_score)。
            if (_state.Phase != GamePhase.Playing)
            {
                _uiService.ShowCollisionMarker(x, y, CollisionResult.Ignored);
                Debug.Log($"[Hit] ignored: the game is not running (phase={_state.Phase})");
                return;
            }

            bool hit = _collisionSolver.TryCollision(collisionPoint, _state.Targets, out Target hitTarget);

            // 着弾位置は当たり外れに関わらず出す。外れが見えないと、
            // 座標変換がずれているのか単に的を外したのかが区別できない。
            var result = hit ? CollisionResult.Scored : CollisionResult.Missed;
            _uiService.ShowCollisionMarker(x, y, result);

            if (!hit)
            {
                Debug.Log($"[Hit] missed: point=({collisionPoint.X:F0}, {collisionPoint.Y:F0}) targets={_state.Targets.Count}");
                return;
            }

            _collisionTargetAction?.Invoke(hitTarget);

            int points = _pointCalculator.CalculatePoint(hitTarget);
            _state.AddScore(points);
            ScoreAdded?.Invoke(points);

            MoveAfterHit(hitTarget);
        }

        /// <summary>
        /// 当たった的を別の場所へ移す。難易度を問わず共通の挙動
        /// (docs/projector_behavior.md)。的は消さず、同じものが動く。
        /// </summary>
        private void MoveAfterHit(Target hitTarget)
        {
            var others = new List<Target>(_state.Targets.Count);
            foreach (var target in _state.Targets)
            {
                if (!ReferenceEquals(target, hitTarget)) others.Add(target);
            }

            hitTarget.MoveTo(_targetGenerator.PickRelocation(hitTarget, others));

            // 当たり判定は移動先で即座に有効になる。見た目が追いつくまでの短い間だけ
            // 表示位置と判定位置がずれるが、演出のための猶予なので許容する。
            _uiService.MoveTarget(hitTarget);
        }
    }
}