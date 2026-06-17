using Struckout.Application;
using Struckout.Infrastructure;
using Struckout.Infrastructure.Network;
using Struckout.Bootstrap;
using UnityEngine;
using VContainer;
using VContainer.Unity;

namespace Struckout.Unity
{
    public class GameLifetimeScope : LifetimeScope
    {
        [SerializeField]
        private UIService _uiService;
        protected override void Configure(IContainerBuilder builder)
        {
            builder.Register<IClientService, TCPClientService>(Lifetime.Singleton);
            builder.Register<IPacketRouter, PacketRouter>(Lifetime.Singleton);
            builder.RegisterComponent(_uiService).As<IUIService>();

            builder.Register<ICollisionSolver,CollisionSolver>(Lifetime.Singleton);
            builder.Register<IPointCalculator, FakePointCalculator>(Lifetime.Singleton);
            builder.Register<ISensorProvider, FakeSensorProvider>(Lifetime.Singleton);
            builder.Register<ITargetGenerator, FakeTargetGenerator>(Lifetime.Singleton);

            builder.Register<NetworkBootstrap>(Lifetime.Singleton);
            builder.Register<GameBootstrap>(Lifetime.Singleton);
            builder.Register<RuntimeContext>(Lifetime.Singleton);

            builder.RegisterEntryPoint<RootBootstrap>();
            
        }
    }
}