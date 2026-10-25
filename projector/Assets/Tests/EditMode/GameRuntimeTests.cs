using System;
using System.Collections.Generic;
using System.Threading;
using System.Threading.Tasks;
using NUnit.Framework;
using Struckout.Application;
using Struckout.Application.Interface;
using Struckout.Domain;
using Struckout.Domain.TargetDefinition;
using Struckout.Infrastructure;
using Tk75Attractions.Struckout.V1;
using Difficulty = Tk75Attractions.Struckout.GameMaster.V1.Difficulty;

namespace Struckout.Tests.Tests.EditMode
{
    public class GameRuntimeTests
    {
        private sealed class Client : IGameMasterClient
        {
            public event Action<Difficulty> GameStarted { add { } remove { } }
            public event Action GameFinished { add { } remove { } }
            public event Action<TimeSpan> RemainingTimeChanged { add { } remove { } }
            public event Action<uint> ScoreChanged { add { } remove { } }
            public event Action ConnectionLost { add { } remove { } }
            public readonly List<string> Calls = new();
            public readonly List<Guid> Hits = new();
            public readonly List<(TargetCoordinate Coordinate, double Radius)> Targets = new();
            public Guid CurrentId;
            public TaskCompletionSource<bool> PendingHit;
            public TaskCompletionSource<Guid?> PendingRegistration;
            public bool RemovalSucceeds = true;
            public Task<bool> ConnectAsync() => Task.FromResult(true);
            public Task DisconnectAsync() => Task.CompletedTask;
            public Task<Guid?> AddTargetAsync(TargetCoordinate coordinate, double radius, CancellationToken token = default)
            {
                Calls.Add("add");
                Targets.Add((coordinate, radius));
                CurrentId = Guid.NewGuid();
                return PendingRegistration?.Task ?? Task.FromResult<Guid?>(CurrentId);
            }
            public Task<bool> RemoveTargetAsync(Guid id, CancellationToken token = default)
            {
                Calls.Add("remove");
                Assert.That(id, Is.EqualTo(Hits[Hits.Count - 1]));
                return Task.FromResult(RemovalSucceeds);
            }
            public Task<bool> HitTargetAsync(Guid id, CancellationToken token = default)
            {
                Calls.Add("hit");
                Hits.Add(id);
                return PendingHit?.Task ?? Task.FromResult(true);
            }
        }

        private sealed class UI : IUIService
        {
            public readonly List<Target> Targets = new();
            public readonly List<Target> Added = new();
            public readonly List<Guid> Removed = new();
            public GamePhase Phase;
            public CollisionResult LastCollision;
            public void SetGamePhase(GamePhase phase) => Phase = phase;
            public void SetRemainingTime(TimeSpan remaining) { }
            public void SetScore(uint score) { }
            public void AddTarget(Target target)
            {
                Assert.That(Targets.Exists(other => other.Id == target.Id), Is.False);
                Targets.Add(target);
                Added.Add(target);
            }
            public bool RemoveTarget(Guid id)
            {
                Assert.That(Targets.RemoveAll(target => target.Id == id), Is.EqualTo(1));
                Removed.Add(id);
                return true;
            }
            public void ShowCollisionMarker(float x, float y, CollisionResult result) => LastCollision = result;
        }

        private static (GameRuntime Runtime, Client Master, UI View) Create()
        {
            var client = new Client();
            var ui = new UI();
            var runtime = new GameRuntime(new CollisionSolver(new CollisionCoordinateTransform()), new TargetGenerator(), ui, client,
                new GameSettings { initialTargetCount = 1 });
            runtime.GameSetup();
            return (runtime, client, ui);
        }

        private static CollisionPoint Center(UI ui) => ui.Targets.Count == 0 ? new CollisionPoint() : new CollisionPoint
        {
            X = ui.Targets[0].Coordinate.X,
            Y = ui.Targets[0].Coordinate.Y,
        };

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
            master.PendingHit = new TaskCompletionSource<bool>();
            var hit = runtime.CollisionDetectedAsync(Center(ui));
            await runtime.CollisionDetectedAsync(Center(ui));
            Assert.That(master.Hits, Has.Count.EqualTo(1));
            runtime.FinishGame();
            master.PendingHit.SetResult(true);
            await hit;
            Assert.That(master.Calls, Is.EqualTo(new[] { "add", "hit" }));
            Assert.That(ui.Added, Has.Count.EqualTo(1));
            Assert.That(ui.Targets, Is.Empty);
            Assert.That(ui.Phase, Is.EqualTo(GamePhase.Finished));
        }

        [Test]
        public async Task Old_registration_response_cannot_overwrite_a_new_games_uuid()
        {
            var (runtime, master, ui) = Create();
            var pending = new TaskCompletionSource<Guid?>();
            master.PendingRegistration = pending;
            var firstStart = runtime.StartGameAsync(Difficulty.Normal);
            await runtime.CollisionDetectedAsync(Center(ui));
            Assert.That(master.Hits, Is.Empty);
            Assert.That(ui.Targets, Is.Empty);
            master.PendingRegistration = null;
            await runtime.StartGameAsync(Difficulty.Hard);
            var newId = master.CurrentId;
            pending.SetResult(Guid.NewGuid());
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
            master.PendingHit = new TaskCompletionSource<bool>();
            master.PendingHit.SetResult(false);
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
            var pending = new TaskCompletionSource<Guid?>();
            master.PendingRegistration = pending;
            var hit = runtime.CollisionDetectedAsync(point);
            Assert.That(ui.Targets, Is.Empty);
            pending.SetResult(null);
            await hit;
            await runtime.CollisionDetectedAsync(point);
            Assert.That(master.Calls, Is.EqualTo(new[] { "add", "hit", "remove", "add" }));
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
            Assert.That(ui.Targets, Is.Empty);
        }

        [Test]
        public async Task Target_is_created_and_drawn_only_after_add_target_returns_its_uuid()
        {
            var (runtime, master, ui) = Create();
            var pending = new TaskCompletionSource<Guid?>();
            master.PendingRegistration = pending;
            var start = runtime.StartGameAsync(Difficulty.Normal);
            Assert.That(master.Calls, Is.EqualTo(new[] { "add" }));
            Assert.That(ui.Added, Is.Empty);
            var id = Guid.NewGuid();
            pending.SetResult(id);
            await start;
            Assert.That(ui.Targets, Has.Count.EqualTo(1));
            Assert.That(ui.Targets[0].Id, Is.EqualTo(id));
        }

        [Test]
        public async Task Failed_initial_registration_does_not_create_a_target()
        {
            var (runtime, master, ui) = Create();
            master.PendingRegistration = new TaskCompletionSource<Guid?>();
            master.PendingRegistration.SetResult(null);
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
            var pending = new TaskCompletionSource<Guid?>();
            master.PendingRegistration = pending;
            var hit = runtime.CollisionDetectedAsync(Center(ui));
            Assert.That(ui.Targets, Is.Empty);
            runtime.FinishGame();
            pending.SetResult(Guid.NewGuid());
            await hit;
            Assert.That(ui.Targets, Is.Empty);
            Assert.That(ui.Added, Has.Count.EqualTo(1));
        }

        [Test]
        public async Task Failed_deletion_removes_the_uncertain_uuid_from_rendering_and_hit_detection()
        {
            var (runtime, master, ui) = Create();
            await runtime.StartGameAsync(Difficulty.Normal);
            var point = Center(ui);
            master.RemovalSucceeds = false;
            await runtime.CollisionDetectedAsync(point);
            await runtime.CollisionDetectedAsync(point);
            Assert.That(ui.Targets, Is.Empty);
            Assert.That(master.Calls, Is.EqualTo(new[] { "add", "hit", "remove" }));
        }
    }
}
