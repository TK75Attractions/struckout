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
            RuntimeContext context,IUIService service
            )
        {
            ICollisionSolver collision = new CollisionSolver();
            IPointCalculator calculator = new FakePointCalculator();
            ISensorProvider sensorProvider = new FakeSensorProvider();
            ITargetGenerator targetGenerator = new FakeTargetGenerator();

            runtime = new(collision,calculator,targetGenerator);
            
            context.PacketRouter.AddCollisionPointAction(runtime.CollisionDetected);
            context.PacketRouter.AddCollisionPointAction(sensorProvider.GetSensorData);

            
            runtime.AddCollisionTargetAction(service.OnCollisionTarget);
            runtime.GameSetup();   
        }
    }
}