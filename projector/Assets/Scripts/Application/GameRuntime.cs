using System;
using System.Collections.Generic;
using System.Threading;
using System.Threading.Tasks;
using JetBrains.Annotations;
using Struckout.Application.Interface;
using Struckout.Domain;
using Struckout.Domain.TargetDefinition;
using Tk75Attractions.Struckout.GameMaster.V1;
using Tk75Attractions.Struckout.V1;
using UnityEngine;
using Target = Struckout.Domain.TargetDefinition.Target;

namespace Struckout.Application
{
    public class GameRuntime
    {
        private readonly ICollisionSolver _collisionSolver;
        private readonly ITargetGenerator _targetGenerator;
        private readonly IUIService _uiService;
        private readonly IGameMasterClient _master;
        private readonly GameSettings _settings;
        private readonly GameRuntimeState _state = new();
        private readonly Dictionary<Target, Guid> _registeredTargets = new();
        private readonly HashSet<Target> _pendingTargets = new();
        private CancellationTokenSource _gameCancellation;

        [UsedImplicitly(ImplicitUseKindFlags.InstantiatedNoFixedConstructorSignature)]
        public GameRuntime(
            ICollisionSolver collisionSolver,
            ITargetGenerator targetGenerator,
            IUIService uiService,
            IGameMasterClient master,
            GameSettings settings)
        {
            _collisionSolver = collisionSolver;
            _targetGenerator = targetGenerator;
            _uiService = uiService;
            _master = master;
            _settings = settings ?? throw new ArgumentNullException(nameof(settings));
        }

        public void GameSetup()
        {
            _state.AddTargets(_targetGenerator, _settings.initialTargetCount, TargetType.Circle);
            _uiService.InstantiateTargets(_state.Targets);
        }

        /// <summary>
        /// Register this machine's targets after GameStarted because the server accepts them only while running.
        /// Called on Unity's main thread so continuations can safely update the board.
        /// </summary>
        public async Task StartGameAsync(Difficulty difficulty)
        {
            CancelPendingOperations();
            _gameCancellation = new CancellationTokenSource();
            var token = _gameCancellation.Token;
            _state.StartGame(difficulty);
            _uiService.SetGamePhase(GamePhase.Playing);

            foreach (var target in _state.Targets)
            {
                var id = await _master.AddTargetAsync(target.Coordinate, target.Radius, token);
                if (token.IsCancellationRequested) return;
                if (id.HasValue) _registeredTargets.Add(target, id.Value);
                else Debug.LogWarning($"[Game] 的を登録できませんでした: {target}");
            }
        }

        public void FinishGame()
        {
            CancelPendingOperations();
            _state.FinishGame();
            _uiService.SetGamePhase(GamePhase.Finished);
        }

        /// <summary>Stop accepting hits after disconnection because the stream cannot restore a session snapshot.</summary>
        public void SuspendGame()
        {
            CancelPendingOperations();
            _state.SuspendGame();
            _uiService.SetGamePhase(GamePhase.Idle);
        }

        private void CancelPendingOperations()
        {
            _gameCancellation?.Cancel();
            _gameCancellation?.Dispose();
            _gameCancellation = null;
            _registeredTargets.Clear();
            _pendingTargets.Clear();
        }

        /// <summary>
        /// Report a registered target's hit, then replace its server registration before moving it locally.
        /// Keep a target busy until replacement completes so overlapping inputs cannot reuse its old UUID.
        /// </summary>
        public async Task CollisionDetectedAsync(CollisionPoint collisionPoint)
        {
            var x = (float)collisionPoint.X;
            var y = (float)collisionPoint.Y;
            if (_state.Phase != GamePhase.Playing)
            {
                _uiService.ShowCollisionMarker(x, y, CollisionResult.Ignored);
                return;
            }
            if (!_collisionSolver.TryCollision(collisionPoint, _state.Targets, out var target))
            {
                _uiService.ShowCollisionMarker(x, y, CollisionResult.Missed);
                return;
            }
            if (!_registeredTargets.TryGetValue(target, out var id) || !_pendingTargets.Add(target))
            {
                _uiService.ShowCollisionMarker(x, y, CollisionResult.Ignored);
                return;
            }

            var token = _gameCancellation.Token;
            try
            {
                var hit = await _master.HitTargetAsync(id, token);
                if (token.IsCancellationRequested) return;
                _uiService.ShowCollisionMarker(x, y, hit ? CollisionResult.Scored : CollisionResult.Ignored);
                if (!hit)
                {
                    Debug.LogWarning($"[Game] 命中を通知できませんでした: {target}");
                    return;
                }

                var removed = await _master.RemoveTargetAsync(id, token);
                if (token.IsCancellationRequested) return;
                if (!removed)
                {
                    // The server may have processed the deletion despite a lost response.
                    _registeredTargets.Remove(target);
                    Debug.LogWarning($"[Game] 的の削除を確認できないため、命中判定を停止します: {target}");
                    return;
                }
                _registeredTargets.Remove(target);
                var others = new List<Target>(_state.Targets);
                others.Remove(target);
                var coordinate = _targetGenerator.PickRelocation(target, others);
                var replacement = await _master.AddTargetAsync(coordinate, target.Radius, token);
                if (token.IsCancellationRequested) return;
                if (!replacement.HasValue)
                {
                    Debug.LogWarning($"[Game] 移動先の的を登録できないため、命中判定を停止します: {target}");
                    return;
                }
                _registeredTargets.Add(target, replacement.Value);
                target.MoveTo(coordinate);
                _uiService.MoveTarget(target);
            }
            finally
            {
                // A subsequent game may already be processing the same local target.
                if (!token.IsCancellationRequested) _pendingTargets.Remove(target);
            }
        }
    }
}
