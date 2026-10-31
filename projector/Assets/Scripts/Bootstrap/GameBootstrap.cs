using Cysharp.Threading.Tasks;
using Struckout.Application;
using Struckout.Domain;
using Struckout.Domain.TargetDefinition;
using System;
using JetBrains.Annotations;
using Struckout.Application.Interface;
using Tk75Attractions.Struckout.GameMaster.V1;
using Tk75Attractions.Struckout.V1;
using Microsoft.Extensions.Logging;
using Target = Struckout.Domain.TargetDefinition.Target;

namespace Struckout.Bootstrap
{
    public class GameBootstrap : IAsyncDestroy
    {
        private readonly ILogger<GameBootstrap> _logger;

        private readonly GameRuntime _runtime;
        private readonly ScreenController _screens;
        private readonly ISensorProvider _sensorProvider;
        private readonly IUIService _service;
        private readonly IGameMasterClient _master;
        private readonly IMainThreadDispatcher _dispatcher;
        private RuntimeContext _context;


        [UsedImplicitly(ImplicitUseKindFlags.InstantiatedNoFixedConstructorSignature)]
        public GameBootstrap(
            GameRuntime runtime,
            ISensorProvider sensorProvider,
            IUIService uiService,
            IGameMasterClient master,
            IMainThreadDispatcher dispatcher,
            ScreenController screens,
            ILogger<GameBootstrap> logger
        )
        {
            _logger = logger ?? throw new ArgumentNullException(nameof(logger));
            _screens = screens ?? throw new ArgumentNullException(nameof(screens));
            _runtime = runtime ?? throw new ArgumentNullException(nameof(runtime));
            _sensorProvider = sensorProvider ?? throw new ArgumentNullException(nameof(sensorProvider));
            _service = uiService ?? throw new ArgumentNullException(nameof(uiService));
            _master = master ?? throw new ArgumentNullException(nameof(master));
            _dispatcher = dispatcher ?? throw new ArgumentNullException(nameof(dispatcher));
        }

        internal void Initialize(
            RuntimeContext context
            )
        {
            if (_service == null) throw new Exception("There are no IUIService in uiService");
            _context = context;

            // 開始通知より先に配置を準備する。登録済みの的だけを後で生成・描画する。
            _runtime.GameSetup();

            context.PacketRouter.OnCollisionReceived += _sensorProvider.GetSensorData;
            _sensorProvider.OnCollisionReceived += OnCollisionDetected;

            // すべて同じ dispatcher を経由し、開始・HUD 更新・終了の順序を保つ。
            _master.RemainingTimeChanged += OnRemainingTimeChanged;
            _master.TotalDurationChanged += OnTotalDurationChanged;
            _master.ScoreChanged += OnScoreChanged;
            _master.ComboChanged += OnComboChanged;
            _master.GameStarted += OnGameStarted;
            _master.GameWillStart += OnGameWillStart;
            _master.GameFinished += OnGameFinished;
            _master.ConnectionLost += OnConnectionLost;
            _master.TargetAdded += OnTargetAdded;
            _master.PocketWatchAdded += OnPocketWatchAdded;
            _master.TargetRemoved += OnTargetRemoved;
            _master.HomeScreenRequested += OnHomeScreenRequested;
        }

        private void OnGameWillStart(uint seconds) => _dispatcher.Enqueue(() =>
        {
            _service.ShowCountdown(seconds);
        });

        private void OnGameStarted(Difficulty difficulty, GameMode mode, PlayerId playerId, TimeSpan duration) =>
            _dispatcher.Enqueue(() =>
            {
                _screens.ShowPlaying(playerId, mode, difficulty);
                _service.StartGameClock(duration);
                ObserveAsync(_runtime.StartGameAsync(difficulty)).Forget();
            });

        private void OnCollisionDetected(CollisionPoint point) =>
            _dispatcher.Enqueue(() => ObserveAsync(_runtime.CollisionDetectedAsync(point)).Forget());

        private void OnTargetAdded(Target target) =>
            _dispatcher.Enqueue(() => _runtime.OnTargetAdded(target));

        private void OnPocketWatchAdded(Guid id, TargetOwnership ownership) =>
            _dispatcher.Enqueue(() => _runtime.OnPocketWatchAdded(id, ownership));

        private void OnTargetRemoved(Guid id) =>
            _dispatcher.Enqueue(() => _runtime.OnTargetRemoved(id));

        private void OnConnectionLost() => _dispatcher.Enqueue(() =>
        {
            _runtime.SuspendGame();
            _screens.Suspend();
        });

        private void OnGameFinished(GameId gameId) => _dispatcher.Enqueue(() =>
        {
            _runtime.FinishGame();
            ObserveAsync(_screens.ShowScoreAsync(gameId)).Forget();
        });

        private void OnHomeScreenRequested() => _dispatcher.Enqueue(() =>
        {
            _runtime.SuspendGame();
            _screens.ShowHome();
        });

        private void OnRemainingTimeChanged(TimeSpan remaining) =>
            _dispatcher.Enqueue(() => _service.SetRemainingTime(remaining));

        private void OnTotalDurationChanged(TimeSpan duration) =>
            _dispatcher.Enqueue(() => _service.SetTotalDuration(duration));

        private void OnComboChanged(uint combo) =>
            _dispatcher.Enqueue(() => _service.SetCombo(combo));

        private void OnScoreChanged(ScoreChange change) =>
            _dispatcher.Enqueue(() =>
            {
                _service.SetScore(change.TotalScore);
                _service.ShowScoreChange(change);
            });

        public UniTask DisposeAsync()
        {
            if (_context == null) return UniTask.CompletedTask;
            _context.PacketRouter.OnCollisionReceived -= _sensorProvider.GetSensorData;
            _sensorProvider.OnCollisionReceived -= OnCollisionDetected;
            _runtime.SuspendGame();
            _screens.Dispose();
            _master.GameStarted -= OnGameStarted;
            _master.GameWillStart -= OnGameWillStart;
            _master.GameFinished -= OnGameFinished;
            _master.ConnectionLost -= OnConnectionLost;
            _master.TargetAdded -= OnTargetAdded;
            _master.PocketWatchAdded -= OnPocketWatchAdded;
            _master.TargetRemoved -= OnTargetRemoved;
            _master.HomeScreenRequested -= OnHomeScreenRequested;
            _master.RemainingTimeChanged -= OnRemainingTimeChanged;
            _master.TotalDurationChanged -= OnTotalDurationChanged;
            _master.ScoreChanged -= OnScoreChanged;
            _master.ComboChanged -= OnComboChanged;
            _context = null;
            return UniTask.CompletedTask;
        }

        private async UniTaskVoid ObserveAsync(UniTask operation)
        {
            try
            {
                await operation;
            }
            catch (OperationCanceledException)
            {
                // The game ended or the connection was lost.
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "処理中に例外が発生しました。");
            }
        }
    }
}
