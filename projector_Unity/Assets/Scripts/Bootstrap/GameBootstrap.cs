using UnityEngine;
using Cysharp.Threading.Tasks;
using Struckout.Application;
using Struckout.Infrastructure;

namespace Struckout.Bootstrap
{
    public class GameBootstrap
    {
        private GameRuntime runtime;
        internal async UniTask Initialize(
            RuntimeContext context, IUIService service
            )
        {
            ICollisionSolver collision = new CollisionSolver();
            IPointCalculator calculator = new FakePointCalculator();
            ISensorProvider sensorProvider = new FakeSensorProvider();
            ITargetGenerator targetGenerator = new FakeTargetGenerator();

            runtime = new(collision,calculator,targetGenerator, service);
            
            context.PacketRouter.OnCollisionReceived += sensorProvider.GetSensorData;
            sensorProvider.OnCollisionReceived += runtime.CollisionDetected;


            
            runtime.AddCollisionTargetAction(service.OnCollisionTarget);
            runtime.GameSetup();   
        }
    }
}