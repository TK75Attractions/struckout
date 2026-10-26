using System;
using System.Collections.Generic;
using System.Reflection;
using Struckout.Domain.TargetDefinition;
using NUnit.Framework;
using Struckout.Domain;
using Struckout.Infrastructure.Grpc;
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
        public void Target_events_supply_server_identity_and_geometry_independently_of_rpc_responses()
        {
            var client = new GrpcGameMasterClient(new NetworkSettings { machineId = 7 });
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
            var client = new GrpcGameMasterClient(new NetworkSettings { machineId = 7 });
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
            var client = new GrpcGameMasterClient(new NetworkSettings { machineId = 7 });
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
            var client = new GrpcGameMasterClient(new NetworkSettings { machineId = 7 });
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
            var client = new GrpcGameMasterClient(new NetworkSettings { machineId = 7 });
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
            var client = new GrpcGameMasterClient(new NetworkSettings { machineId = 7 });
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
            var client = new GrpcGameMasterClient(new NetworkSettings { machineId = machineId });
            PlayerId? received = null;
            client.GameStarted += (_, player) => received = player;
            Dispatch(client, new Event
            {
                GameStarted = new Event.Types.GameStarted
                {
                    Multi = new Event.Types.GameStarted.Types.Multi
                    {
                        Player1 = new Event.Types.GameStarted.Types.PlayerInfo { MachineId = 7, PlayerId = 71 },
                        Player2 = new Event.Types.GameStarted.Types.PlayerInfo { MachineId = 8, PlayerId = 81 },
                    },
                },
            });
            Assert.That(received, Is.EqualTo(new PlayerId(expectedPlayer)));
        }

        [Test]
        public void Single_play_and_finished_events_keep_player_and_game_identity_separate()
        {
            var client = new GrpcGameMasterClient(new NetworkSettings { machineId = 7 });
            PlayerId? player = null;
            GameId? game = null;
            client.GameStarted += (_, id) => player = id;
            client.GameFinished += id => game = id;
            Dispatch(client, new Event
            {
                GameStarted = new Event.Types.GameStarted
                {
                    Single = new Event.Types.GameStarted.Types.Single
                    {
                        Player = new Event.Types.GameStarted.Types.PlayerInfo { MachineId = 7, PlayerId = 71 },
                    },
                },
            });
            Dispatch(client, new Event { GameFinished = new Event.Types.GameFinished { GameId = 42 } });
            Assert.That(player, Is.EqualTo(new PlayerId(71)));
            Assert.That(game, Is.EqualTo(new GameId(42)));
        }

        [Test]
        public void Navigation_events_are_filtered_by_machine_id()
        {
            var client = new GrpcGameMasterClient(new NetworkSettings { machineId = 7 });
            var events = new List<string>();
            client.RankingScreenRequested += () => events.Add("ranking");
            client.HomeScreenRequested += () => events.Add("home");
            foreach (var machine in new uint[] { 8, 7 })
            {
                Dispatch(client, new Event { NavigateToRankingScreenRequested = new Event.Types.NavigateToRankingScreenRequested { MachineId = machine } });
                Dispatch(client, new Event { NavigateToHomeScreenRequsted = new Event.Types.NavigateToHomeScreenRequested { MachineId = machine } });
            }
            Assert.That(events, Is.EqualTo(new[] { "ranking", "home" }));
        }

        [Test]
        public void Missing_or_unrelated_player_information_does_not_start_a_game()
        {
            var client = new GrpcGameMasterClient(new NetworkSettings { machineId = 7 });
            var starts = 0;
            client.GameStarted += (_, __) => starts++;
            Dispatch(client, new Event { GameStarted = new Event.Types.GameStarted() });
            Dispatch(client, new Event
            {
                GameStarted = new Event.Types.GameStarted
                {
                    Single = new Event.Types.GameStarted.Types.Single
                    {
                        Player = new Event.Types.GameStarted.Types.PlayerInfo { MachineId = 8, PlayerId = 71 },
                    },
                },
            });
            Assert.That(starts, Is.Zero);
        }

    }
}
