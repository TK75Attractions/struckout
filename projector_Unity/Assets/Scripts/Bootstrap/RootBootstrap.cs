using System;
using Struckout.Infrastructure;
using UnityEngine;
using Cysharp.Threading.Tasks;

namespace Struckout.Bootstrap
{
    public class RootBootstrap : MonoBehaviour
    {
        private RuntimeContext runtimeContext;

        private void Start()
        {
            Initialize().Forget();
        }

        private async UniTask Initialize()
        {
            
            NetworkBootstrap networkBootstrap = new();
            GameBootstrap gameBootstrap = new();
            PacketRouter packetRouter = new PacketRouter();

            runtimeContext = new(packetRouter);

            await networkBootstrap.Initialize(runtimeContext);
            await gameBootstrap.Initialize(runtimeContext);
        }
        

        private void OnDestroy()
        {
            var destroyList = runtimeContext.destroyEvents;
            foreach (var destroy in destroyList)
            {
                destroy.OnDestroy();
            }
        }
    }
}