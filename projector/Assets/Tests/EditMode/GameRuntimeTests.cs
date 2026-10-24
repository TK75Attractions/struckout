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
using Tk75Attractions.Struckout.GameMaster.V1;
using Tk75Attractions.Struckout.V1;
using Target = Struckout.Domain.TargetDefinition.Target;

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
                return Task.FromResult(true);
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
            public IReadOnlyList<Target> Targets;
            public int Moves;
            public GamePhase Phase;
            public CollisionResult LastCollision;
            public void SetGamePhase(GamePhase phase) => Phase = phase;
            public void SetRemainingTime(TimeSpan remaining) { }
            public void SetScore(uint score) { }
            public void InstantiateTargets(IReadOnlyList<Target> targets) => Targets = targets;
            public void MoveTarget(Target target) => Moves++;
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

        private static CollisionPoint Center(UI ui) => new()
        {
            X = ui.Targets[0].Coordinate.X,
            Y = ui.Targets[0].Coordinate.Y,
        };

        [Test]
        public async Task Hit_replaces_registration_and_uses_the_new_uuid()
        {
            var (runtime, master, ui) = Create();
            Assert.That(master.Calls, Is.Empty);
            await runtime.StartGameAsync(Difficulty.Normal);
            var firstId = master.CurrentId;
            Assert.That(master.Targets[0].Radius, Is.EqualTo(ui.Targets[0].Radius));
            await runtime.CollisionDetectedAsync(Center(ui));
            var secondId = master.CurrentId;
            await runtime.CollisionDetectedAsync(Center(ui));
            Assert.That(master.Calls, Is.EqualTo(new[] { "add", "hit", "remove", "add", "hit", "remove", "add" }));
            Assert.That(master.Hits, Is.EqualTo(new[] { firstId, secondId }));
            Assert.That(secondId, Is.Not.EqualTo(firstId));
            Assert.That(ui.Moves, Is.EqualTo(2));
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
            Assert.That(ui.Moves, Is.Zero);
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
            var coordinate = ui.Targets[0].Coordinate;
            master.PendingHit = new TaskCompletionSource<bool>();
            master.PendingHit.SetResult(false);
            await runtime.CollisionDetectedAsync(Center(ui));
            Assert.That(master.Calls, Is.EqualTo(new[] { "add", "hit" }));
            Assert.That(ui.Targets[0].Coordinate, Is.EqualTo(coordinate));
            Assert.That(ui.Moves, Is.Zero);
            Assert.That(ui.LastCollision, Is.EqualTo(CollisionResult.Ignored));
        }

        [Test]
        public async Task Failed_replacement_stops_hits_instead_of_reusing_the_removed_uuid()
        {
            var (runtime, master, ui) = Create();
            await runtime.StartGameAsync(Difficulty.Normal);
            var pending = new TaskCompletionSource<Guid?>();
            master.PendingRegistration = pending;
            var hit = runtime.CollisionDetectedAsync(Center(ui));
            pending.SetResult(null);
            await hit;
            await runtime.CollisionDetectedAsync(Center(ui));
            Assert.That(master.Calls, Is.EqualTo(new[] { "add", "hit", "remove", "add" }));
            Assert.That(ui.Moves, Is.Zero);
            Assert.That(ui.LastCollision, Is.EqualTo(CollisionResult.Ignored));
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
        }
    }
}
