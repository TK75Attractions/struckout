using Struckout.Infrastructure;
using UnityEngine;
using Cysharp.Threading.Tasks;
using Struckout.Application;
using Struckout.Infrastructure.Network;

namespace Struckout.Bootstrap
{
    public class RootBootstrap : MonoBehaviour
    {
        private RuntimeContext runtimeContext;
        [SerializeField]
        private Transform _uiServiceTransform;
        private IUIService _uiService;


        private void Start()
        {
            Initialize().Forget(UnityEngine.Debug.LogException);
        }

        private async UniTask Initialize()
        {
            
            NetworkBootstrap networkBootstrap = new();
            GameBootstrap gameBootstrap = new();
            IPacketRouter packetRouter = new PacketRouter();
            IClientService clientService = new TCPClientService();
            _uiService = _uiServiceTransform.GetComponent<IUIService>();
            
            if(_uiService == null) throw new System.Exception("There are no IUIService in uiService");

            runtimeContext = new(packetRouter, clientService);

            runtimeContext.AddDestroyEvent(networkBootstrap);

            await networkBootstrap.Initialize(runtimeContext);
            await gameBootstrap.Initialize(runtimeContext, _uiService);
        }
        

        private void OnDestroy()
        {
            if(runtimeContext == null) return;

            var destroyList = runtimeContext.DestroyEvents;

            if (destroyList == null) return;

            foreach (var destroy in destroyList)
            {
                destroy?.OnDestroy().Forget(UnityEngine.Debug.LogException);
            }
        }
    }
}