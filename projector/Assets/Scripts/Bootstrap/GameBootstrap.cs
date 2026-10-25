using Cysharp.Threading.Tasks;
using Struckout.Application;
using System;
using JetBrains.Annotations;
using Struckout.Application.Interface;
using Tk75Attractions.Struckout.GameMaster.V1;
using Tk75Attractions.Struckout.V1;
using System.Threading.Tasks;
using UnityEngine;

namespace Struckout.Bootstrap
{
    public class GameBootstrap : IAsyncDestroy
    {
        private readonly GameRuntime _runtime;
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
            IMainThreadDispatcher dispatcher
        )
        {
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

            // Fake の遅延購読でも、開始通知より先に初期盤面を準備する。
            _runtime.GameSetup();

            context.PacketRouter.OnCollisionReceived += _sensorProvider.GetSensorData;
            _sensorProvider.OnCollisionReceived += OnCollisionDetected;

            // すべて同じ dispatcher を経由し、開始・HUD 更新・終了の順序を保つ。
            _master.RemainingTimeChanged += OnRemainingTimeChanged;
            _master.ScoreChanged += OnScoreChanged;
            _master.GameStarted += OnGameStarted;
            _master.GameFinished += OnGameFinished;
            _master.ConnectionLost += OnConnectionLost;
        }

        private void OnGameStarted(Difficulty difficulty) =>
            _dispatcher.Enqueue(() => ObserveAsync(_runtime.StartGameAsync(difficulty)).Forget());

        private void OnCollisionDetected(CollisionPoint point) =>
            ObserveAsync(_runtime.CollisionDetectedAsync(point)).Forget();

        private void OnConnectionLost() => _dispatcher.Enqueue(_runtime.SuspendGame);

        private void OnGameFinished() =>
            _dispatcher.Enqueue(_runtime.FinishGame);

        private void OnRemainingTimeChanged(TimeSpan remaining) =>
            _dispatcher.Enqueue(() => _service.SetRemainingTime(remaining));

        private void OnScoreChanged(uint totalScore) =>
            _dispatcher.Enqueue(() => _service.SetScore(totalScore));

        public UniTask DisposeAsync()
        {
            if (_context == null) return UniTask.CompletedTask;
            _context.PacketRouter.OnCollisionReceived -= _sensorProvider.GetSensorData;
            _sensorProvider.OnCollisionReceived -= OnCollisionDetected;
            _runtime.SuspendGame();
            _master.GameStarted -= OnGameStarted;
            _master.GameFinished -= OnGameFinished;
            _master.ConnectionLost -= OnConnectionLost;
            _master.RemainingTimeChanged -= OnRemainingTimeChanged;
            _master.ScoreChanged -= OnScoreChanged;
            _context = null;
            return UniTask.CompletedTask;
        }

        private static async UniTaskVoid ObserveAsync(Task operation)
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
                Debug.LogException(ex);
            }
        }
    }
}
