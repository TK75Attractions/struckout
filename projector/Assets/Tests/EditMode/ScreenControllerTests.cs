using GameMode = Tk75Attractions.Struckout.GameMaster.V1.GameMode;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using Cysharp.Threading.Tasks;
using NUnit.Framework;
using Struckout.Application;
using Struckout.Application.Interface;
using Struckout.Domain;
using Struckout.Domain.Ranking;
using Struckout.Domain.TargetDefinition;
using Difficulty = Tk75Attractions.Struckout.GameMaster.V1.Difficulty;

namespace Struckout.Tests.Tests.EditMode
{
    public class ScreenControllerTests
    {
        private sealed class Client : IGameMasterClient
        {
            public event Action<Difficulty, GameMode, PlayerId, TimeSpan> GameStarted { add { } remove { } }
            public event Action<GameId> GameFinished { add { } remove { } }
            public event Action RankingScreenRequested { add { } remove { } }
            public event Action HomeScreenRequested { add { } remove { } }
            public event Action<TimeSpan> RemainingTimeChanged { add { } remove { } }
            public event Action<uint> ScoreChanged { add { } remove { } }
            public event Action<Target> TargetAdded { add { } remove { } }
            public event Action<Guid, TargetOwnership> PocketWatchAdded { add { } remove { } }
            public event Action<Guid> TargetRemoved { add { } remove { } }
            public event Action ConnectionLost { add { } remove { } }
            public uint? Score = 888;
            public IReadOnlyList<RankingEntry> Ranking = Array.Empty<RankingEntry>();
            public IReadOnlyList<RankingEntry> History = Array.Empty<RankingEntry>();
            public UniTaskCompletionSource<uint?> PendingScore;
            public UniTaskCompletionSource<IReadOnlyList<RankingEntry>> PendingRanking;
            public UniTaskCompletionSource<IReadOnlyList<RankingEntry>> PendingHistory;
            public GameId? RequestedGame;
            public PlayerId? RequestedPlayer;
            public uint? RequestedCount;
            public GameMode RankingMode, HistoryMode;
            public Difficulty RankingDifficulty, HistoryDifficulty;
            public CancellationToken ScoreToken;
            public CancellationToken RankingToken;
            public UniTask<bool> ConnectAsync() => UniTask.FromResult(true);
            public UniTask DisconnectAsync() => UniTask.CompletedTask;
            public UniTask<bool> AddTargetAsync(TargetCoordinate coordinate, double radius, CancellationToken token = default) => UniTask.FromResult(true);
            public UniTask<bool> RemoveTargetAsync(Guid id, CancellationToken token = default) => UniTask.FromResult(true);
            public UniTask<bool> HitTargetAsync(Guid id, CancellationToken token = default) => UniTask.FromResult(true);
            public UniTask<uint?> GetGameResultAsync(GameId id, CancellationToken token = default)
            {
                RequestedGame = id;
                ScoreToken = token;
                return PendingScore?.Task ?? UniTask.FromResult(Score);
            }
            public UniTask<IReadOnlyList<RankingEntry>> GetRankingAsync(uint count, GameMode mode, Difficulty difficulty, CancellationToken token = default)
            {
                RequestedCount = count;
                RankingMode = mode;
                RankingDifficulty = difficulty;
                RankingToken = token;
                return PendingRanking?.Task ?? UniTask.FromResult(Ranking);
            }
            public UniTask<IReadOnlyList<RankingEntry>> GetGamesByPlayerAsync(PlayerId id, GameMode mode, Difficulty difficulty, CancellationToken token = default)
            {
                RequestedPlayer = id;
                HistoryMode = mode;
                HistoryDifficulty = difficulty;
                return PendingHistory?.Task ?? UniTask.FromResult(History);
            }
        }

        private sealed class UI : IUIService
        {
            public ProjectorScreen Screen;
            public uint? Score;
            public IReadOnlyList<RankingRow> Rows;
            public string Message;
            public void SetScreen(ProjectorScreen screen) { Screen = screen; Score = null; Rows = null; Message = null; }
            public void ShowGameResult(uint score) { Score = score; Message = null; }
            public void ShowRanking(IReadOnlyList<RankingRow> rows) { Rows = rows; Message = null; }
            public void ShowScreenMessage(string message) => Message = message;
            public void SetRemainingTime(TimeSpan remaining) { }
            public void StartGameClock(TimeSpan duration) { }
            public float PocketWatchDiameter => 200f;
            public void SetScore(uint score) { }
            public void AddTarget(Target target) { }
            public bool RemoveTarget(Guid id) => true;
            public void ShowCollisionMarker(float x, float y, CollisionResult result) { }
        }

        [Test]
        public async Task Screen_flow_uses_the_finished_game_id_and_started_player_id()
        {
            var client = new Client
            {
                Ranking = new[] { new RankingEntry(new GameId(1), 900, new PlayerId(2), "別の人") },
                History = new[] { new RankingEntry(new GameId(42), 888, new PlayerId(7), "本人") },
            };
            var ui = new UI();
            using var screens = new ScreenController(client, ui);
            screens.ShowPlaying(new PlayerId(7), GameMode.Multi, Difficulty.Hard);
            Assert.That(ui.Screen, Is.EqualTo(ProjectorScreen.Playing));
            await screens.ShowScoreAsync(new GameId(42));
            Assert.That(client.RequestedGame, Is.EqualTo(new GameId(42)));
            Assert.That(ui.Screen, Is.EqualTo(ProjectorScreen.Score));
            Assert.That(ui.Score, Is.EqualTo(888));
            await screens.ShowRankingAsync();
            Assert.That(client.RequestedCount, Is.EqualTo(10));
            Assert.That(client.RankingMode, Is.EqualTo(GameMode.Multi));
            Assert.That(client.HistoryMode, Is.EqualTo(GameMode.Multi));
            Assert.That(client.RankingDifficulty, Is.EqualTo(Difficulty.Hard));
            Assert.That(client.HistoryDifficulty, Is.EqualTo(Difficulty.Hard));
            Assert.That(client.RequestedPlayer, Is.EqualTo(new PlayerId(7)));
            Assert.That(ui.Screen, Is.EqualTo(ProjectorScreen.Ranking));
            Assert.That(ui.Rows.Select(row => row.GameId.Value), Is.EqualTo(new uint[] { 1, 42 }));
            Assert.That(ui.Rows[1].IsCurrentGame, Is.True);
            screens.ShowHome();
            Assert.That(ui.Screen, Is.EqualTo(ProjectorScreen.Home));
        }

        [Test]
        public async Task A_late_score_response_cannot_overwrite_home()
        {
            var client = new Client { PendingScore = new UniTaskCompletionSource<uint?>() };
            var ui = new UI();
            using var screens = new ScreenController(client, ui);
            screens.ShowPlaying(new PlayerId(7), GameMode.Multi, Difficulty.Hard);
            var pending = screens.ShowScoreAsync(new GameId(42));
            screens.ShowHome();
            Assert.That(client.ScoreToken.IsCancellationRequested, Is.True);
            client.PendingScore.TrySetResult(123);
            await pending;
            Assert.That(ui.Screen, Is.EqualTo(ProjectorScreen.Home));
            Assert.That(ui.Score, Is.Null);
        }

        [Test]
        public async Task Ranking_requests_run_together_and_late_results_cannot_overwrite_a_new_game()
        {
            var client = new Client
            {
                PendingRanking = new UniTaskCompletionSource<IReadOnlyList<RankingEntry>>(),
                PendingHistory = new UniTaskCompletionSource<IReadOnlyList<RankingEntry>>(),
            };
            var ui = new UI();
            using var screens = new ScreenController(client, ui);
            screens.ShowPlaying(new PlayerId(7), GameMode.Multi, Difficulty.Hard);
            var pending = screens.ShowRankingAsync();
            Assert.That(client.RequestedCount, Is.EqualTo(10));
            Assert.That(client.RankingMode, Is.EqualTo(GameMode.Multi));
            Assert.That(client.HistoryMode, Is.EqualTo(GameMode.Multi));
            Assert.That(client.RankingDifficulty, Is.EqualTo(Difficulty.Hard));
            Assert.That(client.HistoryDifficulty, Is.EqualTo(Difficulty.Hard));
            Assert.That(client.RequestedPlayer, Is.EqualTo(new PlayerId(7)));
            screens.ShowPlaying(new PlayerId(9), GameMode.Multi, Difficulty.Hard);
            Assert.That(client.RankingToken.IsCancellationRequested, Is.True);
            client.PendingRanking.TrySetResult(Array.Empty<RankingEntry>());
            client.PendingHistory.TrySetResult(Array.Empty<RankingEntry>());
            await pending;
            Assert.That(ui.Screen, Is.EqualTo(ProjectorScreen.Playing));
            Assert.That(ui.Rows, Is.Null);
            client.PendingRanking = null;
            client.PendingHistory = null;
            await screens.ShowRankingAsync();
            Assert.That(client.RequestedPlayer, Is.EqualTo(new PlayerId(9)));
        }

        [Test]
        public async Task A_failed_history_request_is_not_presented_as_a_complete_ranking()
        {
            var client = new Client { History = null };
            var ui = new UI();
            using var screens = new ScreenController(client, ui);
            screens.ShowPlaying(new PlayerId(7), GameMode.Multi, Difficulty.Hard);
            await screens.ShowRankingAsync();
            Assert.That(ui.Rows, Is.Null);
            Assert.That(ui.Message, Does.Contain("失敗"));
        }

        [Test]
        public async Task A_failed_result_request_does_not_leave_the_previous_games_score()
        {
            var client = new Client();
            var ui = new UI();
            using var screens = new ScreenController(client, ui);
            await screens.ShowScoreAsync(new GameId(42));
            Assert.That(ui.Score, Is.EqualTo(888));
            client.Score = null;
            await screens.ShowScoreAsync(new GameId(43));
            Assert.That(ui.Score, Is.Null);
            Assert.That(ui.Message, Does.Contain("失敗"));
        }

        [Test]
        public async Task Ranking_without_started_player_information_does_not_guess_an_id()
        {
            var client = new Client();
            var ui = new UI();
            using var screens = new ScreenController(client, ui);
            await screens.ShowRankingAsync();
            Assert.That(client.RequestedPlayer, Is.Null);
            Assert.That(client.RequestedCount, Is.Null);
            Assert.That(ui.Message, Does.Contain("プレイヤー"));
        }

        [Test]
        public async Task Zero_is_a_valid_result_score()
        {
            var client = new Client { Score = 0 };
            var ui = new UI();
            using var screens = new ScreenController(client, ui);
            await screens.ShowScoreAsync(new GameId(42));
            Assert.That(ui.Score, Is.EqualTo(0));
        }
    }
}
