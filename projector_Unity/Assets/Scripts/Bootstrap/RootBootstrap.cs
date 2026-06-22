using UnityEngine;
using Cysharp.Threading.Tasks;
using System;
using System.Threading.Tasks;

namespace Struckout.Bootstrap
{
    public class RootBootstrap : IAsyncDisposable
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

            Initialize().Forget(Debug.LogException);
        }

        private async UniTask Initialize()
        {
            _runtimeContext.AddDestroyEvent(_networkBootstrap);

            await _networkBootstrap.Initialize();
            await _gameBootstrap.Initialize(_runtimeContext);
        }
        

        public async ValueTask DisposeAsync()
        {
            if(_runtimeContext == null) return;

            var destroyList = _runtimeContext.DestroyEvents;

            if (destroyList == null) return;

            foreach (var destroy in destroyList)
            {
                await destroy.DisposeAsync();
            }
        }
    }
}