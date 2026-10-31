using Microsoft.Extensions.Logging;
using Cysharp.Threading.Tasks;
using System;
using JetBrains.Annotations;
using VContainer.Unity;
using UnityEngine;
using Struckout.Domain;
using Struckout.Application;

namespace Struckout.Bootstrap
{
    public class RootBootstrap : IStartable, ITickable, IDisposable
    {
        private readonly ILogger<RootBootstrap> _logger;

        private readonly RuntimeContext _runtimeContext;

        private readonly NetworkBootstrap _networkBootstrap;
        private readonly GameBootstrap _gameBootstrap;
        private readonly GameRuntime _gameRuntime;

        [UsedImplicitly(ImplicitUseKindFlags.InstantiatedNoFixedConstructorSignature)]
        public RootBootstrap(
            NetworkBootstrap networkBootstrap,
            GameBootstrap gameBootstrap,
            GameRuntime gameRuntime,
            RuntimeContext runtimeContext,
            ILogger<RootBootstrap> logger
        )
        {
            _logger = logger ?? throw new ArgumentNullException(nameof(logger));
            _networkBootstrap = networkBootstrap;
            _gameBootstrap = gameBootstrap;
            _gameRuntime = gameRuntime;
            _runtimeContext = runtimeContext;
        }

        public void Start()
        {
            Initialize().Forget(ex => _logger.LogError(ex, "初期化に失敗しました。"));
        }

        public void Tick() => _gameRuntime.AdvanceTargets(Time.deltaTime);

        private async UniTask Initialize()
        {
            _runtimeContext.AddDestroyEvent(_networkBootstrap);
            _runtimeContext.AddDestroyEvent(_gameBootstrap);

            _gameBootstrap.Initialize(_runtimeContext);

            var result = await _networkBootstrap.Initialize();

            switch (result)
            {
                case NetworkConnectionResult.Success:
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
            if (_runtimeContext == null) return;

            var destroyList = _runtimeContext.DestroyEvents;

            if (destroyList == null)
            {
                _logger.LogInformation("The DestroyList was null");
                return;
            }

            foreach (var destroy in destroyList)
            {
                destroy.DisposeAsync();
            }
        }
    }
}
