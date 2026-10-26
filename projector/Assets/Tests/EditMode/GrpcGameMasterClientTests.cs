using System;
using System.Collections.Generic;
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
                TargetId = id.ToString(),
                Target = new ProtoTarget
                {
                    MachineId = machineId,
                    Compass = new ProtoTarget.Types.Compass { X = 100, Y = 200, Radius = 50 },
                },
            },
        };

        [Test]
        public void Target_events_supply_server_identity_and_geometry_independently_of_rpc_responses()
        {
            var client = new GrpcGameMasterClient(new NetworkSettings { machineId = 7 });
            var targets = new List<Target>();
            var removed = new List<Guid>();
            client.TargetAdded += targets.Add;
            client.TargetRemoved += removed.Add;
            var id = Guid.NewGuid();
            client.HandleEvent(Added(id));
            Assert.That(targets, Has.Count.EqualTo(1));
            Assert.That(targets[0].Id, Is.EqualTo(id));
            Assert.That(targets[0].Coordinate.X, Is.EqualTo(100));
            Assert.That(targets[0].Coordinate.Y, Is.EqualTo(200));
            Assert.That(targets[0].Radius, Is.EqualTo(50));
            client.HandleEvent(new Event { TargetRemoved = new Event.Types.TargetRemoved { TargetId = id.ToString() } });
            Assert.That(removed, Is.EqualTo(new[] { id }));
        }

        [Test]
        public void Another_machines_target_is_not_rendered_on_this_machine()
        {
            var client = new GrpcGameMasterClient(new NetworkSettings { machineId = 7 });
            var targets = new List<Target>();
            client.TargetAdded += targets.Add;
            client.HandleEvent(Added(Guid.NewGuid(), 8));
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
            added.TargetAdded.TargetId = id;
            client.HandleEvent(added);
            client.HandleEvent(new Event { TargetRemoved = new Event.Types.TargetRemoved { TargetId = id } });
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
            client.HandleEvent(added);
            Assert.That(targets, Is.Empty);
        }

        [Test]
        public void Missing_target_or_shape_is_ignored()
        {
            var client = new GrpcGameMasterClient(new NetworkSettings { machineId = 7 });
            var targets = new List<Target>();
            client.TargetAdded += targets.Add;
            client.HandleEvent(new Event { TargetAdded = new Event.Types.TargetAdded { TargetId = Guid.NewGuid().ToString() } });
            var added = Added(Guid.NewGuid());
            added.TargetAdded.Target.ClearTargetData();
            client.HandleEvent(added);
            Assert.That(targets, Is.Empty);
        }
    }
}
