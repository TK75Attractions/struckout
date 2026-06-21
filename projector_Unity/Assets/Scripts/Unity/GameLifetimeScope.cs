using Struckout.Application;
using Struckout.Infrastructure;
using Struckout.Infrastructure.Network;
using Struckout.Bootstrap;
using UnityEngine;
using VContainer;
using VContainer.Unity;
using System;

namespace Struckout.Unity
{
    public class GameLifetimeScope : LifetimeScope
    {
        [SerializeField]
        private UIService _uiService;
        [SerializeField]
        private MainThreadDispatcher _dispatcher;

        [SerializeField]
        private RectTransform _targetParent;
        protected override void Configure(IContainerBuilder builder)
        {
            builder.Register<IClientService, FakeClientService>(Lifetime.Singleton);
            builder.Register<IPacketRouter, PacketRouter>(Lifetime.Singleton);
            builder.RegisterComponent(_uiService).As<IUIService>();
            builder.RegisterComponent(_dispatcher).As<IMainThreadDispatcher>();
            builder.Register<GameRuntime>(Lifetime.Singleton);

            builder.Register<ICollisionSolver,CollisionSolver>(Lifetime.Singleton);
            builder.Register<IPointCalculator, FakePointCalculator>(Lifetime.Singleton);
            builder.Register<ISensorProvider, FakeSensorProvider>(Lifetime.Singleton);
            builder.Register<ITargetGenerator, FakeTargetGenerator>(Lifetime.Singleton);

            builder.Register<NetworkBootstrap>(Lifetime.Singleton);
            builder.Register<GameBootstrap>(Lifetime.Singleton);
            builder.Register<RuntimeContext>(Lifetime.Singleton);

            builder.RegisterInstance(
                new UIRoot(_targetParent)
            );

            builder.RegisterEntryPoint<RootBootstrap>();
        
        }
    }
}