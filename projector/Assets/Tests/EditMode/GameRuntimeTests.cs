using Microsoft.Extensions.Logging.Abstractions;
using GameMode = Tk75Attractions.Struckout.GameMaster.V1.GameMode;
using System;
using System.Collections.Generic;
using System.Threading;
using System.Threading.Tasks;
using Cysharp.Threading.Tasks;
using NUnit.Framework;
using Struckout.Application;
using Struckout.Application.Interface;
using Struckout.Domain;
using Struckout.Domain.Ranking;
using Struckout.Domain.TargetDefinition;
using Struckout.Domain.Targets;
using Struckout.Infrastructure;
using Tk75Attractions.Struckout.V1;
using Difficulty = Tk75Attractions.Struckout.GameMaster.V1.Difficulty;

namespace Struckout.Tests.Tests.EditMode
{
    public class GameRuntimeTests
    {
        private sealed class Client : IGameMasterClient
        {
            public event Action<Difficulty, GameMode, PlayerId, TimeSpan> GameStarted { add { } remove { } }
            public event Action<GameId> GameFinished { add { } remove { } }
            public event Action<TimeSpan> RemainingTimeChanged { add { } remove { } }
            public event Action<uint> ScoreChanged { add { } remove { } }
            public event Action<uint> ComboChanged { add { } remove { } }
            public UniTask<bool> ResetComboAsync(CancellationToken token = default)
            {
                Calls.Add("reset");
                return UniTask.FromResult(true);
            }
            public event Action RankingScreenRequested { add { } remove { } }
            public event Action HomeScreenRequested { add { } remove { } }
            public UniTask<uint?> GetGameResultAsync(GameId gameId, CancellationToken token = default) => UniTask.FromResult<uint?>(null);
            public UniTask<IReadOnlyList<RankingEntry>> GetRankingAsync(uint count, GameMode mode, Difficulty difficulty, CancellationToken token = default) => UniTask.FromResult<IReadOnlyList<RankingEntry>>(null);
            public UniTask<IReadOnlyList<RankingEntry>> GetGamesByPlayerAsync(PlayerId playerId, GameMode mode, Difficulty difficulty, CancellationToken token = default) => UniTask.FromResult<IReadOnlyList<RankingEntry>>(null);
            public event Action ConnectionLost { add { } remove { } }
            public event Action<Target> TargetAdded;
            public event Action<Guid, TargetOwnership> PocketWatchAdded { add { } remove { } }
            public event Action<Guid> TargetRemoved;
            public readonly List<string> Calls = new();
            public readonly List<Guid> Hits = new();
            public readonly List<(TargetCoordinate Coordinate, double Radius)> Targets = new();
            public Guid CurrentId;
            public UniTaskCompletionSource<bool> PendingHit;
            public UniTaskCompletionSource<bool> PendingRegistration;
            public bool PublishOnAdd = true;
            public bool PublishOnRemove = true;
            public bool RemovalSucceeds = true;
            public UniTask<bool> ConnectAsync() => UniTask.FromResult(true);
            public UniTask DisconnectAsync() => UniTask.CompletedTask;
            public async UniTask<bool> AddTargetAsync(TargetCoordinate coordinate, double radius, CancellationToken token = default)
            {
                Calls.Add("add");
                Targets.Add((coordinate, radius));
                var id = Guid.NewGuid();
                CurrentId = id;
                var accepted = PendingRegistration == null || await PendingRegistration.Task;
                if (token.IsCancellationRequested) return false;
                if (accepted && PublishOnAdd)
                    EmitAdded(new Target(id, new TargetGeometry(coordinate, TargetType.Compass, (float)radius * 2f), TargetOwnership.Single));
                return accepted;
            }
            public UniTask<bool> RemoveTargetAsync(Guid id, CancellationToken token = default)
            {
                Calls.Add("remove");
                Assert.That(id, Is.EqualTo(Hits[Hits.Count - 1]));
                if (RemovalSucceeds && PublishOnRemove) EmitRemoved(id);
                return UniTask.FromResult(RemovalSucceeds);
            }
            public void EmitAdded(Target target) => TargetAdded?.Invoke(target);
            public void EmitRemoved(Guid id) => TargetRemoved?.Invoke(id);
            public UniTask<bool> HitTargetAsync(Guid id, CancellationToken token = default)
            {
                Calls.Add("hit");
                Hits.Add(id);
                return PendingHit?.Task ?? UniTask.FromResult(true);
            }
        }

        private sealed class UI : IUIService
        {
            public void ShowResultContext(GameMode mode, Difficulty difficulty, uint? rank = null) { }
            public readonly List<Target> Targets = new();
            public readonly List<Target> Added = new();
            public readonly List<Guid> Removed = new();
            public ProjectorScreen Screen;
            public CollisionResult LastCollision;
            public void SetScreen(ProjectorScreen screen) => Screen = screen;
            public void ShowGameResult(uint score) { }
            public void ShowRanking(IReadOnlyList<RankingRow> rows) { }
            public void ShowScreenMessage(string message) { }
            public void SetRemainingTime(TimeSpan remaining) { }
            public void StartGameClock(TimeSpan duration) { }
            public float PocketWatchDiameter => 200f;
            public void SetScore(uint score) { }
            public void SetCombo(uint combo) { }
            public void AddTarget(Target target)
            {
                if (Targets.Exists(other => other.Id == target.Id)) return;
                Targets.Add(target);
                Added.Add(target);
            }
            public bool RemoveTarget(Guid id)
            {
                if (Targets.RemoveAll(target => target.Id == id) == 0) return false;
                Removed.Add(id);
                return true;
            }
            public void ShowCollisionMarker(float x, float y, CollisionResult result) => LastCollision = result;
        }

        private static (GameRuntime Runtime, Client Master, UI View) Create()
        {
            var client = new Client();
            var ui = new UI();
            var runtime = new GameRuntime(new CollisionSolver(new CollisionCoordinateTransform(), NullLogger<CollisionSolver>.Instance), new TargetGenerator(), ui, client,
                new GameSettings { initialTargetCount = 1 }, NullLogger<GameRuntime>.Instance);
            runtime.GameSetup();
            client.TargetAdded += runtime.OnTargetAdded;
            client.TargetRemoved += runtime.OnTargetRemoved;
            return (runtime, client, ui);
        }

        private static CollisionPoint Center(UI ui) => ui.Targets.Count == 0 ? new CollisionPoint() : new CollisionPoint
        {
            X = ui.Targets[0].Coordinate.X,
            Y = ui.Targets[0].Coordinate.Y,
        };

        [Test]
        public async Task Miss_resets_combo_only_while_playing()
        {
            var (runtime, master, ui) = Create();
            var miss = new CollisionPoint { X = -10000, Y = -10000 };
            await runtime.CollisionDetectedAsync(miss);
            Assert.That(master.Calls, Is.Empty);
            await runtime.StartGameAsync(Difficulty.Normal);
            master.Calls.Clear();
            await runtime.CollisionDetectedAsync(miss);
            Assert.That(master.Calls, Is.EqualTo(new[] { "reset" }));
            Assert.That(ui.LastCollision, Is.EqualTo(CollisionResult.Missed));
            runtime.FinishGame();
            await runtime.CollisionDetectedAsync(miss);
            Assert.That(master.Calls, Has.Count.EqualTo(1));
        }

        [Test]
        public async Task Successful_add_rpc_alone_does_not_create_or_draw_a_target()
        {
            var (runtime, master, ui) = Create();
            master.PublishOnAdd = false;
            await runtime.StartGameAsync(Difficulty.Normal);
            Assert.That(ui.Targets, Is.Empty, "TargetAdded を受信するまで状態と描画を変更しない");
        }

        [Test]
        public async Task Hit_replaces_registration_and_uses_the_new_uuid()
        {
            var (runtime, master, ui) = Create();
            Assert.That(master.Calls, Is.Empty);
            Assert.That(ui.Targets, Is.Empty);
            await runtime.StartGameAsync(Difficulty.Normal);
            var original = ui.Targets[0];
            var firstId = original.Id;
            Assert.That(firstId, Is.EqualTo(master.CurrentId));
            Assert.That(master.Targets[0].Radius, Is.EqualTo(original.Radius));
            await runtime.CollisionDetectedAsync(Center(ui));
            var second = ui.Targets[0];
            Assert.That(second, Is.Not.SameAs(original));
            Assert.That(second.Id, Is.EqualTo(master.CurrentId));
            Assert.That(original.Coordinate, Is.EqualTo(master.Targets[0].Coordinate));
            Assert.That(ui.Removed, Is.EqualTo(new[] { firstId }));
            await runtime.CollisionDetectedAsync(Center(ui));
            Assert.That(master.Calls, Is.EqualTo(new[] { "add", "hit", "remove", "add", "hit", "remove", "add" }));
            Assert.That(master.Hits, Is.EqualTo(new[] { firstId, second.Id }));
            Assert.That(ui.Targets, Has.Count.EqualTo(1));
            Assert.That(ui.Added, Has.Count.EqualTo(3));
            Assert.That(ui.Removed, Is.EqualTo(new[] { firstId, second.Id }));
            Assert.That(master.Targets[2].Coordinate, Is.EqualTo(ui.Targets[0].Coordinate));
        }

        [Test]
        public async Task Pending_hit_rejects_duplicate_input_and_cannot_move_after_finish()
        {
            var (runtime, master, ui) = Create();
            await runtime.StartGameAsync(Difficulty.Hard);
            master.PendingHit = new UniTaskCompletionSource<bool>();
            var hit = runtime.CollisionDetectedAsync(Center(ui));
            await runtime.CollisionDetectedAsync(Center(ui));
            Assert.That(master.Hits, Has.Count.EqualTo(1));
            runtime.FinishGame();
            master.PendingHit.TrySetResult(true);
            await hit;
            Assert.That(master.Calls, Is.EqualTo(new[] { "add", "hit" }));
            Assert.That(ui.Added, Has.Count.EqualTo(1));
            Assert.That(ui.Targets, Is.Empty);
            await runtime.CollisionDetectedAsync(Center(ui));
            Assert.That(ui.LastCollision, Is.EqualTo(CollisionResult.Ignored));
        }

        [Test]
        public async Task Old_registration_response_cannot_overwrite_a_new_games_uuid()
        {
            var (runtime, master, ui) = Create();
            var pending = new UniTaskCompletionSource<bool>();
            master.PendingRegistration = pending;
            var firstStart = runtime.StartGameAsync(Difficulty.Normal);
            await runtime.CollisionDetectedAsync(Center(ui));
            Assert.That(master.Hits, Is.Empty);
            Assert.That(ui.Targets, Is.Empty);
            master.PendingRegistration = null;
            await runtime.StartGameAsync(Difficulty.Hard);
            var newId = master.CurrentId;
            pending.TrySetResult(true);
            await firstStart;
            await runtime.CollisionDetectedAsync(Center(ui));
            Assert.That(master.Hits, Is.EqualTo(new[] { newId }));
        }

        [Test]
        public async Task Failed_hit_keeps_the_original_registration_and_position()
        {
            var (runtime, master, ui) = Create();
            await runtime.StartGameAsync(Difficulty.Normal);
            var original = ui.Targets[0];
            master.PendingHit = new UniTaskCompletionSource<bool>();
            master.PendingHit.TrySetResult(false);
            await runtime.CollisionDetectedAsync(Center(ui));
            Assert.That(master.Calls, Is.EqualTo(new[] { "add", "hit" }));
            Assert.That(ui.Targets[0], Is.SameAs(original));
            Assert.That(ui.Removed, Is.Empty);
            Assert.That(ui.LastCollision, Is.EqualTo(CollisionResult.Ignored));
        }

        [Test]
        public async Task Failed_replacement_stops_hits_instead_of_reusing_the_removed_uuid()
        {
            var (runtime, master, ui) = Create();
            await runtime.StartGameAsync(Difficulty.Normal);
            var point = Center(ui);
            var pending = new UniTaskCompletionSource<bool>();
            master.PendingRegistration = pending;
            var hit = runtime.CollisionDetectedAsync(point);
            Assert.That(ui.Targets, Is.Empty);
            pending.TrySetResult(false);
            await hit;
            await runtime.CollisionDetectedAsync(point);
            Assert.That(master.Calls, Is.EqualTo(new[] { "add", "hit", "remove", "add", "reset" }));
            Assert.That(ui.Targets, Is.Empty);
            Assert.That(ui.Added, Has.Count.EqualTo(1));
        }

        [Test]
        public async Task Hits_before_start_after_finish_and_after_disconnect_are_ignored()
        {
            var (runtime, master, ui) = Create();
            await runtime.CollisionDetectedAsync(Center(ui));
            await runtime.StartGameAsync(Difficulty.Normal);
            runtime.FinishGame();
            await runtime.CollisionDetectedAsync(Center(ui));
            await runtime.StartGameAsync(Difficulty.Normal);
            runtime.SuspendGame();
            await runtime.CollisionDetectedAsync(Center(ui));
            Assert.That(master.Hits, Is.Empty);
            Assert.That(ui.LastCollision, Is.EqualTo(CollisionResult.Ignored));
            Assert.That(ui.Targets, Has.Count.EqualTo(1));
        }

        [Test]
        public async Task Event_before_rpc_response_is_applied_without_waiting_for_the_response()
        {
            var (runtime, master, ui) = Create();
            var pending = new UniTaskCompletionSource<bool>();
            master.PendingRegistration = pending;
            master.PublishOnAdd = false;
            var start = runtime.StartGameAsync(Difficulty.Normal);
            Assert.That(ui.Added, Is.Empty);
            var id = Guid.NewGuid();
            var geometry = master.Targets[0];
            master.EmitAdded(new Target(id, new TargetGeometry(geometry.Coordinate, TargetType.Compass, (float)geometry.Radius * 2f), TargetOwnership.Single));
            Assert.That(ui.Targets[0].Id, Is.EqualTo(id));
            Assert.That(start.Status, Is.EqualTo(UniTaskStatus.Pending));
            pending.TrySetResult(true);
            await start;
            Assert.That(ui.Targets, Has.Count.EqualTo(1));
        }

        [Test]
        public async Task Failed_initial_registration_does_not_create_a_target()
        {
            var (runtime, master, ui) = Create();
            master.PendingRegistration = new UniTaskCompletionSource<bool>();
            master.PendingRegistration.TrySetResult(false);
            await runtime.StartGameAsync(Difficulty.Normal);
            await runtime.CollisionDetectedAsync(Center(ui));
            Assert.That(ui.Added, Is.Empty);
            Assert.That(master.Hits, Is.Empty);
        }

        [Test]
        public async Task Replacement_response_after_finish_does_not_create_an_entity()
        {
            var (runtime, master, ui) = Create();
            await runtime.StartGameAsync(Difficulty.Normal);
            var pending = new UniTaskCompletionSource<bool>();
            master.PendingRegistration = pending;
            var hit = runtime.CollisionDetectedAsync(Center(ui));
            Assert.That(ui.Targets, Is.Empty);
            runtime.FinishGame();
            pending.TrySetResult(true);
            await hit;
            Assert.That(ui.Targets, Is.Empty);
            Assert.That(ui.Added, Has.Count.EqualTo(1));
        }

        [Test]
        public async Task Failed_deletion_keeps_the_target_until_its_removal_event_and_blocks_further_hits()
        {
            var (runtime, master, ui) = Create();
            await runtime.StartGameAsync(Difficulty.Normal);
            var id = ui.Targets[0].Id;
            var point = Center(ui);
            master.RemovalSucceeds = false;
            await runtime.CollisionDetectedAsync(point);
            await runtime.CollisionDetectedAsync(point);
            Assert.That(ui.Targets, Has.Count.EqualTo(1));
            Assert.That(ui.Removed, Is.Empty);
            Assert.That(master.Calls, Is.EqualTo(new[] { "add", "hit", "remove" }));
            master.EmitRemoved(id);
            Assert.That(ui.Targets, Is.Empty);
        }

        [Test]
        public async Task Successful_removal_response_alone_does_not_remove_a_target()
        {
            var (runtime, master, ui) = Create();
            await runtime.StartGameAsync(Difficulty.Normal);
            var original = ui.Targets[0];
            master.PublishOnRemove = false;
            master.PublishOnAdd = false;
            await runtime.CollisionDetectedAsync(Center(ui));
            Assert.That(ui.Targets[0], Is.SameAs(original));
            Assert.That(ui.Removed, Is.Empty);
            await runtime.CollisionDetectedAsync(Center(ui));
            Assert.That(master.Hits, Has.Count.EqualTo(1));
            master.EmitRemoved(original.Id);
            Assert.That(ui.Targets, Is.Empty);
        }

        [Test]
        public async Task Finish_clears_all_targets_but_disconnect_keeps_them()
        {
            var (runtime, master, ui) = Create();
            await runtime.StartGameAsync(Difficulty.Normal);
            var id = ui.Targets[0].Id;
            var another = new Target(Guid.NewGuid(), ui.Targets[0].Geometry, TargetOwnership.All);
            master.EmitAdded(another);
            runtime.FinishGame();
            Assert.That(ui.Targets, Is.Empty);
            Assert.That(ui.Removed, Is.EquivalentTo(new[] { id, another.Id }));

            await runtime.StartGameAsync(Difficulty.Normal);
            var nextId = ui.Targets[0].Id;
            runtime.SuspendGame();
            Assert.That(ui.Targets[0].Id, Is.EqualTo(nextId));
            master.PublishOnAdd = false;
            await runtime.StartGameAsync(Difficulty.Normal);
            Assert.That(ui.Targets[0].Id, Is.EqualTo(nextId));
            master.EmitRemoved(nextId);
            Assert.That(ui.Targets, Is.Empty);
        }

        [Test]
        public async Task Duplicate_addition_and_unknown_removal_events_do_not_corrupt_state()
        {
            var (runtime, master, ui) = Create();
            await runtime.StartGameAsync(Difficulty.Normal);
            master.EmitAdded(ui.Targets[0]);
            master.EmitRemoved(Guid.NewGuid());
            Assert.That(ui.Targets, Has.Count.EqualTo(1));
            Assert.That(ui.Added, Has.Count.EqualTo(1));
            Assert.That(ui.Removed, Is.Empty);
        }
        [Test]
        public async Task All_target_hits_do_not_request_removal_or_replacement()
        {
            var (runtime, master, ui) = Create();
            master.PublishOnAdd = false;
            await runtime.StartGameAsync(Difficulty.Normal);
            var shared = new Target(Guid.NewGuid(),
                new TargetGeometry(new TargetCoordinate(500, 500), TargetType.Compass, 100), TargetOwnership.All);
            master.EmitAdded(shared);
            master.Calls.Clear();
            await runtime.CollisionDetectedAsync(Center(ui));
            await runtime.CollisionDetectedAsync(Center(ui));
            Assert.That(master.Calls, Is.EqualTo(new[] { "hit", "hit" }));
            Assert.That(ui.Targets[0], Is.SameAs(shared));
            Assert.That(ui.Removed, Is.Empty);
            master.EmitRemoved(shared.Id);
            Assert.That(ui.Targets, Is.Empty);
        }

        [TestCase(TargetOwnership.Single)]
        [TestCase(TargetOwnership.All)]
        public async Task Pocket_watch_uses_local_fixed_size_and_waits_for_server_replacement(TargetOwnership ownership)
        {
            var (runtime, master, ui) = Create();
            master.PublishOnAdd = false;
            await runtime.StartGameAsync(Difficulty.Normal);
            var id = Guid.NewGuid();
            runtime.OnPocketWatchAdded(id, ownership);
            var watch = ui.Targets[0];
            Assert.That(watch.Type, Is.EqualTo(TargetType.PocketWatch));
            Assert.That(watch.Diameter, Is.EqualTo(ui.PocketWatchDiameter));
            Assert.That(watch.Ownership, Is.EqualTo(ownership));
            Assert.That(watch.Coordinate.X, Is.InRange(watch.Radius, 1920 - watch.Radius));
            Assert.That(watch.Coordinate.Y, Is.InRange(watch.Radius, 1080 - watch.Radius));
            runtime.OnPocketWatchAdded(id, ownership);
            Assert.That(ui.Added, Has.Count.EqualTo(1));
            master.Calls.Clear();
            await runtime.CollisionDetectedAsync(Center(ui));
            await runtime.CollisionDetectedAsync(Center(ui));
            Assert.That(master.Calls, Is.EqualTo(new[] { "hit" }));
            Assert.That(ui.Targets[0], Is.SameAs(watch));
            master.EmitRemoved(id);
            Assert.That(ui.Targets, Is.Empty);
            var replacementId = Guid.NewGuid();
            runtime.OnPocketWatchAdded(replacementId, ownership);
            Assert.That(ui.Targets[0].Id, Is.EqualTo(replacementId));
            Assert.That(ui.Targets[0].Diameter, Is.EqualTo(watch.Diameter));
            Assert.That(ui.Targets[0].Coordinate, Is.Not.EqualTo(watch.Coordinate));
            runtime.FinishGame();
            Assert.That(ui.Targets, Is.Empty);
        }

        [Test]
        public async Task Pocket_watch_placement_avoids_existing_targets()
        {
            var (runtime, master, ui) = Create();
            await runtime.StartGameAsync(Difficulty.Normal);
            var compass = ui.Targets[0];
            runtime.OnPocketWatchAdded(Guid.NewGuid(), TargetOwnership.All);
            var watch = ui.Targets[1];
            var dx = watch.Coordinate.X - compass.Coordinate.X;
            var dy = watch.Coordinate.Y - compass.Coordinate.Y;
            Assert.That(Math.Sqrt(dx * dx + dy * dy), Is.GreaterThan(watch.Radius + compass.Radius));
        }

    }
}
