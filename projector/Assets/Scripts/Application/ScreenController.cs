using System;
using System.Threading;
using Cysharp.Threading.Tasks;
using JetBrains.Annotations;
using Struckout.Application.Interface;
using Struckout.Domain;

namespace Struckout.Application
{
    public sealed class ScreenController : IDisposable
    {
        private readonly IGameMasterClient _master;
        private readonly IUIService _ui;
        private PlayerId? _playerId;
        private GameId? _gameId;
        private CancellationTokenSource _request;

        [UsedImplicitly(ImplicitUseKindFlags.InstantiatedNoFixedConstructorSignature)]
        public ScreenController(IGameMasterClient master, IUIService ui)
        {
            _master = master ?? throw new ArgumentNullException(nameof(master));
            _ui = ui ?? throw new ArgumentNullException(nameof(ui));
        }

        public void ShowPlaying(PlayerId playerId)
        {
            CancelRequest();
            _playerId = playerId;
            _gameId = null;
            _ui.SetScreen(ProjectorScreen.Playing);
        }

        public async UniTask ShowScoreAsync(GameId gameId)
        {
            var token = BeginRequest(ProjectorScreen.Score);
            _gameId = gameId;
            _ui.ShowScreenMessage("結果を取得しています…");
            try
            {
                var score = await _master.GetGameResultAsync(gameId, token);
                if (token.IsCancellationRequested) return;
                if (score.HasValue) _ui.ShowGameResult(score.Value);
                else _ui.ShowScreenMessage("結果の取得に失敗しました。");
            }
            catch (OperationCanceledException) when (token.IsCancellationRequested) { }
        }

        public async UniTask ShowRankingAsync()
        {
            var token = BeginRequest(ProjectorScreen.Ranking);
            if (!_playerId.HasValue)
            {
                _ui.ShowScreenMessage("プレイヤー情報を取得できませんでした。");
                return;
            }
            var playerId = _playerId.Value;
            var gameId = _gameId;
            _ui.ShowScreenMessage("ランキングを取得しています…");
            try
            {
                var rankingTask = _master.GetRankingAsync(RankingResults.TopItemCount, token);
                var historyTask = _master.GetGamesByPlayerAsync(playerId, token);
                var (ranking, history) = await UniTask.WhenAll(rankingTask, historyTask);
                if (token.IsCancellationRequested) return;
                if (ranking == null || history == null)
                {
                    _ui.ShowScreenMessage("ランキングの取得に失敗しました。");
                    return;
                }
                _ui.ShowRanking(RankingResults.Merge(ranking, history, playerId, gameId));
            }
            catch (OperationCanceledException) when (token.IsCancellationRequested) { }
        }

        public void ShowHome()
        {
            Suspend();
            _playerId = null;
            _gameId = null;
        }

        public void Suspend()
        {
            CancelRequest();
            _ui.SetScreen(ProjectorScreen.Home);
        }

        private CancellationToken BeginRequest(ProjectorScreen screen)
        {
            CancelRequest();
            _request = new CancellationTokenSource();
            _ui.SetScreen(screen);
            return _request.Token;
        }

        private void CancelRequest()
        {
            _request?.Cancel();
            _request?.Dispose();
            _request = null;
        }

        public void Dispose() => CancelRequest();
    }
}
