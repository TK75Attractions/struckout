using UnityEngine;
using Cysharp.Threading.Tasks;
using System;
using VContainer.Unity;

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

            await _networkBootstrap.Initialize();
            await _gameBootstrap.Initialize(_runtimeContext);
        }
        

        public void Dispose()
        {
            Debug.Log("DisposeAsync");
            if(_runtimeContext == null) return;
            Debug.Log("DisposeAsync2");

            var destroyList = _runtimeContext.DestroyEvents;

            if (destroyList == null) return;

            foreach (var destroy in destroyList)
            {
                destroy.DisposeAsync();
            }
        }
    }
}