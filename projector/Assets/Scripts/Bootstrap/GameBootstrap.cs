using Cysharp.Threading.Tasks;
using Struckout.Application;
using System;

namespace Struckout.Bootstrap
{
    public class GameBootstrap
    {
        private readonly GameRuntime _runtime;
        private readonly ISensorProvider _sensorProvider;
        private readonly IUIService _service;


        public GameBootstrap(
            GameRuntime runtime,
            ISensorProvider sensorProvider,
            IUIService uiService
        )
        {
            _runtime = runtime ?? throw new ArgumentNullException(nameof(runtime));
            _sensorProvider = sensorProvider ?? throw new ArgumentNullException(nameof(sensorProvider));
            _service = uiService ?? throw new ArgumentNullException(nameof(uiService));
        }

        internal async UniTask Initialize(
            RuntimeContext context
            )
        {
            if(_service == null) throw new Exception("There are no IUIService in uiService");
            
            context.PacketRouter.OnCollisionReceived += _sensorProvider.GetSensorData;
            _sensorProvider.OnCollisionReceived += _runtime.CollisionDetected;

            _runtime.AddCollisionTargetAction(_service.OnCollisionTarget);
            _runtime.GameSetup();   
        }
    }
}