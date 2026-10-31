using Microsoft.Extensions.Logging.Abstractions;
using GameMode = Tk75Attractions.Struckout.GameMaster.V1.GameMode;
using PlayerInfo = Tk75Attractions.Struckout.GameMaster.V1.PlayerInfo;
using System;
using System.Collections.Generic;
using System.Reflection;
using Struckout.Domain.TargetDefinition;
using NUnit.Framework;
using Struckout.Domain;
using Struckout.Infrastructure.Grpc;
using Struckout.Application.Interface;
using Event = Tk75Attractions.Struckout.GameMaster.V1.Event;
using ProtoTarget = Tk75Attractions.Struckout.GameMaster.V1.Target;
using Target = Struckout.Domain.TargetDefinition.Target;

namespace Struckout.Tests.Tests.EditMode
{
    public class GrpcGameMasterClientTests
    {
        private static Event Added(Guid id, uint machineId = 7) => new()
        {
            TargetAdded = new Event.Types.TargetAdded
            {
                Target = new ProtoTarget
                {
                    TargetId = id.ToString(),
                    Single = new ProtoTarget.Types.OwnerSingle { MachineId = machineId },
                    Compass = new ProtoTarget.Types.Compass { X = 100, Y = 200, Radius = 50 },
                },
            },
        };

        private static void Dispatch(GrpcGameMasterClient client, Event received)
        {
            var method = typeof(GrpcGameMasterClient).GetMethod("HandleEvent", BindingFlags.Instance | BindingFlags.NonPublic);
            Assert.That(method, Is.Not.Null);
            method.Invoke(client, new object[] { received });
        }

        [Test]
        public void Score_events_preserve_the_increment_and_reason()
        {
            var client = new GrpcGameMasterClient(new NetworkSettings { machineId = 7 }, NullLogger<GrpcGameMasterClient>.Instance);
            var changes = new List<ScoreChange>();
            client.ScoreChanged += changes.Add;
            Dispatch(client, new Event
            {
                GameStarted = new Event.Types.GameStarted
                {
                    PlayerInfo = new PlayerInfo
                    {
                        Single = new PlayerInfo.Types.Single
                        {
                            Player = new PlayerInfo.Types.PlayerItem { MachineId = 7, PlayerId = 71 },
                        },
                    },
                },
            });
            var targetId = Guid.NewGuid();
            Dispatch(client, new Event
            {
                GameScoreChanged = new Event.Types.GameScoreChanged
                {
                    TotalScore = 300,
                    Diff = 300,
                    TargetHit = new Event.Types.GameScoreChanged.Types.TargetHit { TargetId = targetId.ToString() },
                },
            });
            Dispatch(client, new Event
            {
                GameScoreChanged = new Event.Types.GameScoreChanged
                {
                    TotalScore = 800,
                    Diff = 500,
                    Combo = new Google.Protobuf.WellKnownTypes.Empty(),
                },
            });
            Assert.That(changes, Has.Count.EqualTo(2));
            Assert.That(changes[0].Reason, Is.EqualTo(ScoreReason.TargetHit));
            Assert.That(changes[0].TargetId, Is.EqualTo(targetId));
            Assert.That(changes[0].Difference, Is.EqualTo(300));
            Assert.That(changes[1].Reason, Is.EqualTo(ScoreReason.Combo));
            Assert.That(changes[1].Difference, Is.EqualTo(500));
            Assert.That(changes[1].TotalScore, Is.EqualTo(800));
        }

        [Test]
        public void Combo_events_are_forwarded_only_during_play()
        {
            var client = new GrpcGameMasterClient(new NetworkSettings { machineId = 7 }, NullLogger<GrpcGameMasterClient>.Instance);
            var combos = new List<uint>();
            client.ComboChanged += combos.Add;
            var changed = new Event { ComboChanged = new Event.Types.ComboChanged { Combo = 3 } };
            Dispatch(client, changed);
            Assert.That(combos, Is.Empty);
            Dispatch(client, new Event
            {
                GameStarted = new Event.Types.GameStarted
                {
                    PlayerInfo = new PlayerInfo
                    {
                        Single = new PlayerInfo.Types.Single
                        {
                            Player = new PlayerInfo.Types.PlayerItem { MachineId = 7, PlayerId = 71 },
                        },
                    },
                },
            });
            Dispatch(client, changed);
            Dispatch(client, new Event { ComboChanged = new Event.Types.ComboChanged { Combo = 0 } });
            Dispatch(client, new Event { GameFinished = new Event.Types.GameFinished { GameId = 1 } });
            Dispatch(client, changed);
            Assert.That(combos, Is.EqualTo(new uint[] { 3, 0 }));
        }

        [Test]
        public void Target_events_supply_server_identity_and_geometry_independently_of_rpc_responses()
        {
            var client = new GrpcGameMasterClient(new NetworkSettings { machineId = 7 }, NullLogger<GrpcGameMasterClient>.Instance);
            var targets = new List<Target>();
            var removed = new List<Guid>();
            client.TargetAdded += targets.Add;
            client.TargetRemoved += removed.Add;
            var id = Guid.NewGuid();
            Dispatch(client, Added(id));
            Assert.That(targets, Has.Count.EqualTo(1));
            Assert.That(targets[0].Id, Is.EqualTo(id));
            Assert.That(targets[0].Coordinate.X, Is.EqualTo(100));
            Assert.That(targets[0].Coordinate.Y, Is.EqualTo(200));
            Assert.That(targets[0].Radius, Is.EqualTo(50));
            Dispatch(client, new Event { TargetRemoved = new Event.Types.TargetRemoved { TargetId = id.ToString() } });
            Assert.That(removed, Is.EqualTo(new[] { id }));
        }

        [Test]
        public void Another_machines_target_is_not_rendered_on_this_machine()
        {
            var client = new GrpcGameMasterClient(new NetworkSettings { machineId = 7 }, NullLogger<GrpcGameMasterClient>.Instance);
            var targets = new List<Target>();
            client.TargetAdded += targets.Add;
            Dispatch(client, Added(Guid.NewGuid(), 8));
            Assert.That(targets, Is.Empty);
        }

        [TestCase("")]
        [TestCase("invalid")]
        [TestCase("00000000-0000-0000-0000-000000000000")]
        public void Invalid_target_uuids_are_ignored(string id)
        {
            var client = new GrpcGameMasterClient(new NetworkSettings { machineId = 7 }, NullLogger<GrpcGameMasterClient>.Instance);
            var targets = new List<Target>();
            var removed = new List<Guid>();
            client.TargetAdded += targets.Add;
            client.TargetRemoved += removed.Add;
            var added = Added(Guid.NewGuid());
            added.TargetAdded.Target.TargetId = id;
            Dispatch(client, added);
            Dispatch(client, new Event { TargetRemoved = new Event.Types.TargetRemoved { TargetId = id } });
            Assert.That(targets, Is.Empty);
            Assert.That(removed, Is.Empty);
        }

        [TestCase(0f)]
        [TestCase(-1f)]
        [TestCase(float.NaN)]
        [TestCase(float.PositiveInfinity)]
        [TestCase(float.MaxValue)]
        public void Invalid_target_geometry_does_not_create_an_entity(float radius)
        {
            var client = new GrpcGameMasterClient(new NetworkSettings { machineId = 7 }, NullLogger<GrpcGameMasterClient>.Instance);
            var targets = new List<Target>();
            client.TargetAdded += targets.Add;
            var added = Added(Guid.NewGuid());
            added.TargetAdded.Target.Compass.Radius = radius;
            Dispatch(client, added);
            Assert.That(targets, Is.Empty);
        }

        [Test]
        public void Missing_target_or_shape_is_ignored()
        {
            var client = new GrpcGameMasterClient(new NetworkSettings { machineId = 7 }, NullLogger<GrpcGameMasterClient>.Instance);
            var targets = new List<Target>();
            client.TargetAdded += targets.Add;
            Dispatch(client, new Event { TargetAdded = new Event.Types.TargetAdded() });
            var added = Added(Guid.NewGuid());
            added.TargetAdded.Target.ClearTargetData();
            Dispatch(client, added);
            Assert.That(targets, Is.Empty);
        }
        [Test]
        public void All_target_retains_shared_ownership()
        {
            var client = new GrpcGameMasterClient(new NetworkSettings { machineId = 7 }, NullLogger<GrpcGameMasterClient>.Instance);
            var targets = new List<Target>();
            client.TargetAdded += targets.Add;
            var added = Added(Guid.NewGuid(), 8);
            added.TargetAdded.Target.All = new ProtoTarget.Types.OwnerAll();
            Dispatch(client, added);
            Assert.That(targets, Has.Count.EqualTo(1));
            Assert.That(targets[0].Ownership, Is.EqualTo(TargetOwnership.All));
        }

        [TestCase(7u, 71u)]
        [TestCase(8u, 81u)]
        public void Multi_play_selects_the_player_for_this_machine(uint machineId, uint expectedPlayer)
        {
            var client = new GrpcGameMasterClient(new NetworkSettings { machineId = machineId }, NullLogger<GrpcGameMasterClient>.Instance);
            PlayerId? received = null;
            GameMode? receivedMode = null;
            client.GameStarted += (_, mode, player, duration) => { received = player; receivedMode = mode; };
            Dispatch(client, new Event
            {
                GameStarted = new Event.Types.GameStarted
                {
                    PlayerInfo = new PlayerInfo
                    {
                        Multi = new PlayerInfo.Types.Multi
                        {
                            Player1 = new PlayerInfo.Types.PlayerItem { MachineId = 7, PlayerId = 71 },
                            Player2 = new PlayerInfo.Types.PlayerItem { MachineId = 8, PlayerId = 81 },
                        },
                    },
                },
            });
            Assert.That(received, Is.EqualTo(new PlayerId(expectedPlayer)));
            Assert.That(receivedMode, Is.EqualTo(GameMode.Multi));
        }

        [Test]
        public void Single_play_and_finished_events_keep_player_and_game_identity_separate()
        {
            var client = new GrpcGameMasterClient(new NetworkSettings { machineId = 7 }, NullLogger<GrpcGameMasterClient>.Instance);
            PlayerId? player = null;
            GameId? game = null;
            client.GameStarted += (_, mode, id, duration) => player = id;
            client.GameFinished += id => game = id;
            Dispatch(client, new Event
            {
                GameStarted = new Event.Types.GameStarted
                {
                    PlayerInfo = new PlayerInfo
                    {
                        Single = new PlayerInfo.Types.Single
                        {
                            Player = new PlayerInfo.Types.PlayerItem { MachineId = 7, PlayerId = 71 },
                        },
                    },
                },
            });
            Dispatch(client, new Event { GameFinished = new Event.Types.GameFinished { GameId = 42 } });
            Assert.That(player, Is.EqualTo(new PlayerId(71)));
            Assert.That(game, Is.EqualTo(new GameId(42)));
        }

        [Test]
        public void Home_navigation_event_is_filtered_by_machine_id()
        {
            var client = new GrpcGameMasterClient(new NetworkSettings { machineId = 7 }, NullLogger<GrpcGameMasterClient>.Instance);
            var events = new List<string>();
            client.HomeScreenRequested += () => events.Add("home");
            foreach (var machine in new uint[] { 8, 7 })
            {
                Dispatch(client, new Event { NavigateToHomeScreenRequsted = new Event.Types.NavigateToHomeScreenRequested { MachineId = machine } });
            }
            Assert.That(events, Is.EqualTo(new[] { "home" }));
        }

        [Test]
        public void Missing_or_unrelated_player_information_does_not_start_a_game()
        {
            var client = new GrpcGameMasterClient(new NetworkSettings { machineId = 7 }, NullLogger<GrpcGameMasterClient>.Instance);
            var starts = 0;
            client.GameStarted += (_, mode, __, duration) => starts++;
            Dispatch(client, new Event { GameStarted = new Event.Types.GameStarted() });
            Dispatch(client, new Event
            {
                GameStarted = new Event.Types.GameStarted
                {
                    PlayerInfo = new PlayerInfo
                    {
                        Single = new PlayerInfo.Types.Single
                        {
                            Player = new PlayerInfo.Types.PlayerItem { MachineId = 8, PlayerId = 71 },
                        },
                    },
                },
            });
            Assert.That(starts, Is.Zero);
        }

        [TestCase(false, 7u, 1)]
        [TestCase(false, 8u, 0)]
        [TestCase(true, 8u, 1)]
        public void Pocket_watch_event_preserves_identity_and_filters_ownership(bool shared, uint machineId, int count)
        {
            var client = new GrpcGameMasterClient(new NetworkSettings { machineId = 7 }, NullLogger<GrpcGameMasterClient>.Instance);
            var watches = new List<Guid>();
            var compasses = new List<Target>();
            TargetOwnership? owner = null;
            client.PocketWatchAdded += (id, ownership) => { watches.Add(id); owner = ownership; };
            client.TargetAdded += compasses.Add;
            var id = Guid.NewGuid();
            var added = Added(id, machineId);
            added.TargetAdded.Target.PocketWatch = new ProtoTarget.Types.PocketWatch();
            if (shared) added.TargetAdded.Target.All = new ProtoTarget.Types.OwnerAll();
            Dispatch(client, added);
            Assert.That(watches, Has.Count.EqualTo(count));
            Assert.That(compasses, Is.Empty);
            if (count > 0)
            {
                Assert.That(watches[0], Is.EqualTo(id));
                Assert.That(owner, Is.EqualTo(shared ? TargetOwnership.All : TargetOwnership.Single));
            }
        }

        [Test]
        public void Game_start_passes_the_server_duration_in_seconds()
        {
            var client = new GrpcGameMasterClient(new NetworkSettings { machineId = 7 }, NullLogger<GrpcGameMasterClient>.Instance);
            TimeSpan? duration = null;
            client.GameStarted += (_, mode, player, value) => duration = value;
            Dispatch(client, new Event
            {
                GameStarted = new Event.Types.GameStarted
                {
                    GameDurSecs = 120,
                    PlayerInfo = new PlayerInfo
                    {
                        Single = new PlayerInfo.Types.Single
                        {
                            Player = new PlayerInfo.Types.PlayerItem { MachineId = 7, PlayerId = 71 },
                        },
                    },
                },
            });
            Assert.That(duration, Is.EqualTo(TimeSpan.FromSeconds(120)));
        }

    }
}
