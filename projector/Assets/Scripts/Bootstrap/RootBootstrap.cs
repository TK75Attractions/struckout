using UnityEngine;
using Cysharp.Threading.Tasks;
using System;
using VContainer.Unity;
using Struckout.Domain;

namespace Struckout.Bootstrap
{
    public class RootBootstrap : IStartable, IDisposable
    {
        private RuntimeContext _runtimeContext;

        private NetworkBootstrap _networkBootstrap;
        private GameBootstrap _gameBootstrap;
        public RootBootstrap(
            NetworkBootstrap networkBootstrap,
            GameBootstrap gameBootstrap,
            RuntimeContext runtimeContext
        )
        {
            _networkBootstrap = networkBootstrap;
            _gameBootstrap = gameBootstrap;
            _runtimeContext = runtimeContext;
        }

        public void Start()
        {
            Initialize().Forget(Debug.LogException);
        }

        private async UniTask Initialize()
        {
            _runtimeContext.AddDestroyEvent(_networkBootstrap);

            NetworkConnectionResult result = await _networkBootstrap.Initialize();

            switch (result)
            {
                case NetworkConnectionResult.Success:
                    await _gameBootstrap.Initialize(_runtimeContext);
                    break;
                case NetworkConnectionResult.ClientConnectFailed:
                    throw new Exception("Client connect failed");
                case NetworkConnectionResult.MasterConnectFailed:
                    throw new Exception("Master connect failed");
                case NetworkConnectionResult.InvalidConfiguration:
                    throw new Exception("There are no enough injections");
                default:
                    throw new InvalidOperationException("Unknown result");
            }
            
        }
        

        public void Dispose()
        {
            Debug.Log("DisposeAsync");
            if(_runtimeContext == null) return;
            Debug.Log("DisposeAsync2");

            var destroyList = _runtimeContext.DestroyEvents;

            if (destroyList == null)
            {
                Debug.Log("The DestroyList is null");
            }

            foreach (var destroy in destroyList)
            {
                Debug.Log("Dispose");
                destroy.DisposeAsync();
            }
        }
    }
}