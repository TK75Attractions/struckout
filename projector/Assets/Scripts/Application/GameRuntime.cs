using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using JetBrains.Annotations;
using Struckout.Application.Interface;
using Struckout.Domain;
using Struckout.Domain.TargetDefinition;
using Tk75Attractions.Struckout.V1;
using UnityEngine;
using Difficulty = Tk75Attractions.Struckout.GameMaster.V1.Difficulty;

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
        private readonly HashSet<Guid> _pendingTargets = new();
        private IReadOnlyList<TargetGeometry> _initialGeometry;
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
            // Geometry has no identity and cannot participate in rendering or hit detection.
            _initialGeometry = _targetGenerator.GenerateTargets(
                _settings.initialTargetCount, TargetType.Circle, Array.Empty<TargetGeometry>());
        }

        /// <summary>
        /// Register geometry before constructing any local entities because game_master owns their identity.
        /// Called on Unity's main thread so continuations can safely update the board.
        /// </summary>
        public async Task StartGameAsync(Difficulty difficulty)
        {
            if (_initialGeometry == null) throw new InvalidOperationException("GameSetup must run before starting a game.");
            CancelPendingOperations();
            ClearLocalTargets();
            _gameCancellation = new CancellationTokenSource();
            var token = _gameCancellation.Token;
            _state.StartGame(difficulty);
            _uiService.SetGamePhase(GamePhase.Playing);

            foreach (var geometry in _initialGeometry)
            {
                await RegisterTargetAsync(geometry, token);
                if (token.IsCancellationRequested) return;
            }
        }

        private async Task RegisterTargetAsync(TargetGeometry geometry, CancellationToken token)
        {
            var id = await _master.AddTargetAsync(geometry.Coordinate, geometry.Radius, token);
            if (token.IsCancellationRequested) return;
            if (!id.HasValue)
            {
                Debug.LogWarning("[Game] 的を登録できなかったため、生成しません。");
                return;
            }
            var target = new Target(id.Value, geometry);
            _state.AddTarget(target);
            _uiService.AddTarget(target);
        }

        private void RemoveLocalTarget(Guid id)
        {
            _state.RemoveTarget(id);
            if (!_uiService.RemoveTarget(id))
            {
                Debug.LogWarning($"Tried to remove the target which doesn't exist: id = {id}");
            }
        }

        private void ClearLocalTargets()
        {
            foreach (var target in new List<Target>(_state.Targets)) RemoveLocalTarget(target.Id);
        }

        public void FinishGame()
        {
            CancelPendingOperations();
            ClearLocalTargets();
            _state.FinishGame();
            _uiService.SetGamePhase(GamePhase.Finished);
        }

        /// <summary>Stop accepting hits after disconnection because the stream cannot restore a session snapshot.</summary>
        public void SuspendGame()
        {
            CancelPendingOperations();
            ClearLocalTargets();
            _state.SuspendGame();
            _uiService.SetGamePhase(GamePhase.Idle);
        }

        private void CancelPendingOperations()
        {
            _gameCancellation?.Cancel();
            _gameCancellation?.Dispose();
            _gameCancellation = null;
            _pendingTargets.Clear();
        }

        /// <summary>
        /// Replace a hit target by removing its UUID and registering new geometry as a separate entity.
        /// Keep its UUID busy while awaiting the server so overlapping inputs cannot report duplicate hits.
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
            if (!_pendingTargets.Add(target.Id))
            {
                _uiService.ShowCollisionMarker(x, y, CollisionResult.Ignored);
                return;
            }

            var token = _gameCancellation.Token;
            try
            {
                var hit = await _master.HitTargetAsync(target.Id, token);
                if (token.IsCancellationRequested) return;
                _uiService.ShowCollisionMarker(x, y, hit ? CollisionResult.Scored : CollisionResult.Ignored);
                if (!hit)
                {
                    Debug.LogWarning($"[Game] 命中を通知できませんでした: {target}");
                    return;
                }

                var removed = await _master.RemoveTargetAsync(target.Id, token);
                if (token.IsCancellationRequested) return;
                // If the deletion response was lost, stop displaying or accepting hits for the uncertain UUID.
                RemoveLocalTarget(target.Id);
                if (!removed)
                {
                    Debug.LogWarning($"[Game] 的の削除を確認できないため、再生成を停止します: {target}");
                    return;
                }
                var others = _state.Targets.Select(other => other.Geometry).ToList();
                var coordinate = _targetGenerator.PickRelocation(target.Geometry, others);
                await RegisterTargetAsync(new TargetGeometry(coordinate, target.Type, target.Size), token);
            }
            finally
            {
                // A subsequent game may already be processing another set of UUIDs.
                if (!token.IsCancellationRequested) _pendingTargets.Remove(target.Id);
            }
        }
    }
}
