using Tk75Attractions.Struckout.GameMaster.V1;
using System;
using System.Threading;
using System.Linq;
using Cysharp.Threading.Tasks;
using JetBrains.Annotations;
using Struckout.Application.Interface;
using Struckout.Domain;
using Struckout.Domain.Ranking;

namespace Struckout.Application
{
    public sealed class ScreenController : IDisposable
    {
        private readonly IGameMasterClient _master;
        private readonly IUIService _ui;
        private readonly bool _simpleMode;
        private PlayerId? _playerId;
        private GameId? _gameId;
        private GameMode _mode;
        private Difficulty _difficulty;
        private CancellationTokenSource _request;

        [UsedImplicitly(ImplicitUseKindFlags.InstantiatedNoFixedConstructorSignature)]
        public ScreenController(IGameMasterClient master, IUIService ui, NetworkSettings settings)
        {
            _master = master ?? throw new ArgumentNullException(nameof(master));
            _ui = ui ?? throw new ArgumentNullException(nameof(ui));
            _simpleMode = (settings ?? throw new ArgumentNullException(nameof(settings))).simpleMode;
        }

        public void ShowPlaying(PlayerId playerId, GameMode mode, Difficulty difficulty)
        {
            CancelRequest();
            _playerId = playerId;
            _mode = mode;
            _difficulty = difficulty;
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
                if (!score.HasValue)
                {
                    _ui.ShowScreenMessage("結果の取得に失敗しました。");
                    return;
                }
                ShowResultRankAsync(gameId, token).Forget();
                await _ui.ShowGameResultAsync(score.Value, token);
                if (_simpleMode) return;
                await UniTask.Delay(TimeSpan.FromSeconds(1), DelayType.Realtime, cancellationToken: token);
                if (!token.IsCancellationRequested) await ShowRankingAsync();
            }
            catch (OperationCanceledException) when (token.IsCancellationRequested) { }
        }

        private async UniTaskVoid ShowResultRankAsync(GameId gameId, CancellationToken token)
        {
            try
            {
                var rankingTask = _master.GetRankingAsync(RankingResults.TopItemCount, _mode, _difficulty, token);
                var historyTask = _playerId.HasValue
                    ? _master.GetGamesByPlayerAsync(_playerId.Value, _mode, _difficulty, token)
                    : UniTask.FromResult<System.Collections.Generic.IReadOnlyList<RankingEntry>>(null);
                var (ranking, history) = await UniTask.WhenAll(rankingTask, historyTask);
                if (token.IsCancellationRequested) return;
                var entries = (history ?? System.Array.Empty<RankingEntry>())
                    .Concat(ranking ?? System.Array.Empty<RankingEntry>()).ToArray();
                _ui.ShowResultContext(_mode, _difficulty, RankingResults.GetRank(entries, gameId));
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
                var rankingTask = _master.GetRankingAsync(RankingResults.TopItemCount, _mode, _difficulty, token);
                var historyTask = _master.GetGamesByPlayerAsync(playerId, _mode, _difficulty, token);
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
            _ui.ShowResultContext(_mode, _difficulty);
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
