using UnityEngine;
using Cysharp.Threading.Tasks;
using VContainer;

namespace Struckout.Bootstrap
{
    public class RootBootstrap : MonoBehaviour
    {
        private RuntimeContext _runtimeContext;

        private NetworkBootstrap _networkBootstrap;
        private GameBootstrap _gameBootstrap;

        private void Start()
        {
            Initialize().Forget(UnityEngine.Debug.LogException);
        }

        [Inject]
        public void Construct(
            NetworkBootstrap networkBootstrap,
            GameBootstrap gameBootstrap,
            RuntimeContext runtimeContext
        )
        {
            _networkBootstrap = networkBootstrap;
            _gameBootstrap = gameBootstrap;
            _runtimeContext = runtimeContext;
        }

        private async UniTask Initialize()
        {
            _runtimeContext.AddDestroyEvent(_networkBootstrap);

            await _networkBootstrap.Initialize(_runtimeContext);
            await _gameBootstrap.Initialize(_runtimeContext);
        }
        

        private void OnDestroy()
        {
            if(_runtimeContext == null) return;

            var destroyList = _runtimeContext.DestroyEvents;

            if (destroyList == null) return;

            foreach (var destroy in destroyList)
            {
                destroy?.OnDestroy().Forget(UnityEngine.Debug.LogException);
            }
        }
    }
}