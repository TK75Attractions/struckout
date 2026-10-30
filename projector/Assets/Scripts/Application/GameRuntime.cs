using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading;
using Cysharp.Threading.Tasks;
using JetBrains.Annotations;
using Struckout.Application.Interface;
using Struckout.Domain;
using Struckout.Domain.TargetDefinition;
using Struckout.Domain.Targets;
using Tk75Attractions.Struckout.V1;
using Microsoft.Extensions.Logging;
using Difficulty = Tk75Attractions.Struckout.GameMaster.V1.Difficulty;

namespace Struckout.Application
{
    public class GameRuntime
    {
        private readonly ILogger<GameRuntime> _logger;

        private readonly ICollisionSolver _collisionSolver;
        private readonly TargetGenerator _targetGenerator;
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
            TargetGenerator targetGenerator,
            IUIService uiService,
            IGameMasterClient master,
            GameSettings settings,
            ILogger<GameRuntime> logger)
        {
            _logger = logger ?? throw new ArgumentNullException(nameof(logger));
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
                _settings.initialTargetCount, TargetType.Compass, Array.Empty<TargetGeometry>());
        }

        /// <summary>
        /// Requests registration of initial geometry; only TargetAdded events create local entities.
        /// Called on Unity's main thread so continuations can safely update the board.
        /// </summary>
        public async UniTask StartGameAsync(Difficulty difficulty)
        {
            if (_initialGeometry == null) throw new InvalidOperationException("GameSetup must run before starting a game.");
            CancelPendingOperations();
            _gameCancellation = new CancellationTokenSource();
            var token = _gameCancellation.Token;
            _state.StartGame(difficulty);

            foreach (var geometry in _initialGeometry)
            {
                await RegisterTargetAsync(geometry, token);
                if (token.IsCancellationRequested) return;
            }
        }

        private async UniTask RegisterTargetAsync(TargetGeometry geometry, CancellationToken token)
        {
            var accepted = await _master.AddTargetAsync(geometry.Coordinate, geometry.Radius, token);
            if (!accepted && !token.IsCancellationRequested)
                _logger.LogWarning("的の登録を要求できませんでした。");
        }

        /// <summary>Applies an authoritative TargetAdded event, independently of the registration RPC response.</summary>
        public void OnTargetAdded(Target target)
        {
            if (!_state.AddTarget(target)) _logger.LogWarning("Notified target {TargetId} already exists in the GameRuntimeState", target.Id);
            _uiService.AddTarget(target);
        }

        /// <summary>Places a server-created watch locally because its event carries no geometry.</summary>
        public void OnPocketWatchAdded(Guid id, TargetOwnership ownership)
        {
            if (_state.Targets.Any(target => target.Id == id)) return;
            var diameter = _uiService.PocketWatchDiameter;
            // Reserve the whole artwork canvas, including the crown above the collision circle.
            var placement = new TargetGeometry(default, TargetType.PocketWatch,
                diameter * PocketWatchLayout.CanvasToDialRatio);
            var others = _state.Targets.Select(target => target.Geometry).ToList();
            // Watch events can arrive before the initial Compass registration events.
            if (_state.Targets.All(target => target.Type != TargetType.Compass) && _initialGeometry != null)
                others.AddRange(_initialGeometry);
            var coordinate = _targetGenerator.PickRelocation(placement, others);
            OnTargetAdded(new Target(id, new TargetGeometry(coordinate, TargetType.PocketWatch, diameter), ownership));
        }

        /// <summary>Applies an authoritative TargetRemoved event; RPC results never remove local targets.</summary>
        public void OnTargetRemoved(Guid id)
        {
            _pendingTargets.Remove(id);
            if (!_state.RemoveTarget(id)) _logger.LogWarning("[Game] There's no target with the notified id: {Id}", id);
            if (!_uiService.RemoveTarget(id))
                _logger.LogWarning("削除通知に対応する描画がありません: {Id}", id);
        }

        /// <summary>Clears remaining local targets when the game finishes.</summary>
        public void FinishGame()
        {
            CancelPendingOperations();
            _state.FinishGame();
            foreach (var id in _state.Targets.Select(target => target.Id).ToList())
                OnTargetRemoved(id);
        }

        /// <summary>Stops accepting hits after disconnection because the stream cannot restore a session snapshot.</summary>
        public void SuspendGame()
        {
            CancelPendingOperations();
            _state.SuspendGame();
        }

        private void CancelPendingOperations()
        {
            _gameCancellation?.Cancel();
            _gameCancellation?.Dispose();
            _gameCancellation = null;
            _pendingTargets.Clear();
        }

        /// <summary>
        /// Requests replacement of a hit target; target events control local state during play.
        /// Keeps its UUID busy while awaiting the server so overlapping inputs cannot report duplicate hits.
        /// </summary>
        public async UniTask CollisionDetectedAsync(CollisionPoint collisionPoint)
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
                var missToken = _gameCancellation.Token;
                if (!await _master.ResetComboAsync(missToken) && !missToken.IsCancellationRequested)
                    _logger.LogWarning("コンボをリセットできませんでした。");
                return;
            }
            if (!_pendingTargets.Add(target.Id))
            {
                _uiService.ShowCollisionMarker(x, y, CollisionResult.Ignored);
                return;
            }

            var token = _gameCancellation.Token;
            var removalRequested = false;
            try
            {
                var hit = await _master.HitTargetAsync(target.Id, token);
                if (token.IsCancellationRequested) return;
                _uiService.ShowCollisionMarker(x, y, hit ? CollisionResult.Scored : CollisionResult.Ignored);
                if (!hit)
                {
                    _logger.LogWarning("命中を通知できませんでした: {Target}", target);
                    return;
                }

                if (target.Type == TargetType.PocketWatch)
                {
                    // The server replaces watches even in single play. Keep this UUID busy until its event.
                    removalRequested = true;
                    return;
                }
                if (target.Ownership == TargetOwnership.All) return;

                removalRequested = true;
                var removed = await _master.RemoveTargetAsync(target.Id, token);
                if (token.IsCancellationRequested) return;
                if (!removed)
                {
                    _logger.LogWarning("的の削除を確認できないため、再生成を停止します: {Target}", target);
                    return;
                }
                var others = _state.Targets.Where(other => other.Id != target.Id).Select(other => other.Geometry).ToList();
                var coordinate = _targetGenerator.PickRelocation(target.Geometry, others);
                await RegisterTargetAsync(new TargetGeometry(coordinate, target.Type, target.Size), token);
            }
            finally
            {
                // Once deletion is requested, only TargetRemoved releases this UUID.
                // A lost RPC response must not allow another hit on a possibly deleted target.
                if (!token.IsCancellationRequested && !removalRequested) _pendingTargets.Remove(target.Id);
            }
        }
    }
}
