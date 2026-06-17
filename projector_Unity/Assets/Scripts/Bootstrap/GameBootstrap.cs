using Cysharp.Threading.Tasks;
using Struckout.Application;

namespace Struckout.Bootstrap
{
    public class GameBootstrap
    {
        private GameRuntime runtime;
        private readonly ICollisionSolver collision;
        private readonly IPointCalculator calculator;
        private readonly ISensorProvider sensorProvider;
        private readonly ITargetGenerator targetGenerator;
        private readonly IUIService service;


        public GameBootstrap(
            ICollisionSolver Collision,
            IPointCalculator PointCalculator,
            ISensorProvider SensorProvider,
            ITargetGenerator TargetGenerator,
            IUIService UIService
        )
        {
            collision = Collision;
            calculator = PointCalculator;
            sensorProvider = SensorProvider;
            targetGenerator = TargetGenerator;
            service = UIService;
        }

        internal async UniTask Initialize(
            RuntimeContext context
            )
        {
            if(service == null) throw new System.Exception("There are no IUIService in uiService");

            runtime = new(collision, calculator,targetGenerator, service);
            
            context.PacketRouter.OnCollisionReceived += sensorProvider.GetSensorData;
            sensorProvider.OnCollisionReceived += runtime.CollisionDetected;

            runtime.AddCollisionTargetAction(service.OnCollisionTarget);
            runtime.GameSetup();   
        }
    }
}