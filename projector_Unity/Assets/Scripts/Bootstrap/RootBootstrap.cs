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

            runtimeContext.AddDestroyEvent(networkBootstrap);

            await networkBootstrap.Initialize(runtimeContext);
            await gameBootstrap.Initialize(runtimeContext);
        }
        

        private void OnDestroy()
        {
            if(runtimeContext == null) return;

            var destroyList = runtimeContext.destroyEvents;

            if (destroyList == null) return;

            foreach (var destroy in destroyList)
            {
                destroy?.OnDestroy().Forget();
            }
        }
    }
}