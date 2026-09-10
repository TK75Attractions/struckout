using Cysharp.Threading.Tasks;
using Struckout.Application;
using System;
using Tk75Attractions.Struckout.V1;
using UnityEngine;

namespace Struckout.Bootstrap
{
    public class GameBootstrap
    {
        private readonly GameRuntime _runtime;
        private readonly ISensorProvider _sensorProvider;
        private readonly IUIService _service;
        private readonly IGameMasterClient _master;


        public GameBootstrap(
            GameRuntime runtime,
            ISensorProvider sensorProvider,
            IUIService uiService,
            IGameMasterClient master
        )
        {
            _runtime = runtime ?? throw new ArgumentNullException(nameof(runtime));
            _sensorProvider = sensorProvider ?? throw new ArgumentNullException(nameof(sensorProvider));
            _service = uiService ?? throw new ArgumentNullException(nameof(uiService));
            _master = master ?? throw new ArgumentNullException(nameof(master));
        }

        internal async UniTask Initialize(
            RuntimeContext context
            )
        {
            if(_service == null) throw new Exception("There are no IUIService in uiService");

            context.PacketRouter.OnCollisionReceived += _sensorProvider.GetSensorData;
            _sensorProvider.OnCollisionReceived += _runtime.CollisionDetected;

            // 的の見た目の更新は GameRuntime が IUIService を直接叩く。
            // ここでは得点の送信だけを繋ぐ。
            _runtime.ScoreAdded += OnScoreAdded;

            // ゲームの開始と終了は game_master のイベントストリームから来る。
            // 残り時間 (GameTimeLimitNotify) も流れてくるが、projector と touchpanel の
            // どちらが出すかが未決なので、まだ拾っていない。
            _master.GameStarted += OnGameStarted;
            _master.GameFinished += OnGameFinished;

            _runtime.GameSetup();
        }

        private void OnGameStarted(Difficulty difficulty) =>
            _runtime.StartGame(difficulty);

        private void OnGameFinished() =>
            _runtime.FinishGame();

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
