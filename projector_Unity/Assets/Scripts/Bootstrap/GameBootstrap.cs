using Cysharp.Threading.Tasks;
using Struckout.Application;
using System;

namespace Struckout.Bootstrap
{
    public class GameBootstrap
    {
        private GameRuntime runtime;
        private readonly ISensorProvider sensorProvider;
        private readonly IUIService service;


        public GameBootstrap(
            GameRuntime runtime,
            ISensorProvider SensorProvider,
            IUIService UIService
        )
        {
            this.runtime = runtime;
            sensorProvider = SensorProvider;
            service = UIService;
        }

        internal async UniTask Initialize(
            RuntimeContext context
            )
        {
            if(service == null) throw new Exception("There are no IUIService in uiService");
            
            context.PacketRouter.OnCollisionReceived += sensorProvider.GetSensorData;
            sensorProvider.OnCollisionReceived += runtime.CollisionDetected;

            runtime.AddCollisionTargetAction(service.OnCollisionTarget);
            runtime.GameSetup();   
        }
    }
}