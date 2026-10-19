using Cysharp.Threading.Tasks;
using Struckout.Application;
using System;
using JetBrains.Annotations;
using Struckout.Application.Interface;
using Tk75Attractions.Struckout.V1;
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

        internal async UniTask Initialize(
            RuntimeContext context
            )
        {
            if(_service == null) throw new Exception("There are no IUIService in uiService");
            _context = context;

            // Fake の遅延購読でも、開始通知より先に初期盤面を準備する。
            _runtime.GameSetup();

            context.PacketRouter.OnCollisionReceived += _sensorProvider.GetSensorData;
            _sensorProvider.OnCollisionReceived += _runtime.CollisionDetected;

            // 的の見た目の更新は GameRuntime が IUIService を直接叩く。
            // ここでは得点の送信だけを繋ぐ。
            _runtime.ScoreAdded += OnScoreAdded;

            // すべて同じ dispatcher を経由し、開始・HUD 更新・終了の順序を保つ。
            _master.RemainingTimeChanged += OnRemainingTimeChanged;
            _master.ScoreChanged += OnScoreChanged;
            _master.GameStarted += OnGameStarted;
            _master.GameFinished += OnGameFinished;
        }

        private void OnGameStarted(Difficulty difficulty) =>
            _dispatcher.Enqueue(() => _runtime.StartGame(difficulty));

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
            _sensorProvider.OnCollisionReceived -= _runtime.CollisionDetected;
            _runtime.ScoreAdded -= OnScoreAdded;
            _master.GameStarted -= OnGameStarted;
            _master.GameFinished -= OnGameFinished;
            _master.RemainingTimeChanged -= OnRemainingTimeChanged;
            _master.ScoreChanged -= OnScoreChanged;
            _context = null;
            return UniTask.CompletedTask;
        }

        private void OnScoreAdded(int points)
        {
            SendScoreAsync(points).Forget();
        }

        /// <summary>
        /// game_master には得点の増分を送る (AddScoreRequest.score_to_add)。
        /// 送信に失敗してもゲームは続けたいので、ここで握って警告に留める。
        /// </summary>
        private async UniTaskVoid SendScoreAsync(int points)
        {
            if (points <= 0)
            {
                Debug.LogWarning($"[Network] skipped sending a non-positive score delta ({points})");
                return;
            }

            try
            {
                bool sent = await _master.AddScoreAsync(points);
                if (!sent) Debug.LogWarning($"[Network] failed to send score delta {points}");
            }
            catch (Exception ex)
            {
                Debug.LogException(ex);
            }
        }
    }
}
