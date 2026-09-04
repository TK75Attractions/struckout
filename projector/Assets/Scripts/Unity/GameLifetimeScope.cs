using Struckout.Application;
using Struckout.Domain;
using Struckout.Infrastructure;
using Struckout.Bootstrap;
using UnityEngine;
using VContainer;
using VContainer.Unity;
using Tk75Attractions.Struckout.V1;

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

        [SerializeField]
        [Tooltip("Fake にすると ball_tracker / game_master なしで起動できる。コマンドライン引数と環境変数で上書きできる。")]
        private NetworkSettings _networkSettings = new();

        [SerializeField]
        [Tooltip("ball_tracker から届く物理座標 (m) を的の描画座標に直す係数。実測して合わせること。")]
        private CollisionCoordinateTransform _collisionTransform = new();

        [SerializeField]
        [Tooltip("的の数とクールダウン時間。")]
        private GameSettings _gameSettings = new();

        protected override void Configure(IContainerBuilder builder)
        {
            var networkSettings = NetworkSettingsResolver.Resolve(_networkSettings);
            builder.RegisterInstance(networkSettings);

            // 物理座標 -> 描画座標 の係数。SensorProvider が使う。
            Debug.Log($"[Collision] transform {_collisionTransform}");
            builder.RegisterInstance(_collisionTransform);

            Debug.Log($"[Game] {_gameSettings}");
            builder.RegisterInstance(_gameSettings);

            if (networkSettings.Mode == NetworkMode.Fake)
            {
                builder.Register<IClientService<ProjectorPacket>, FakeClientService>(Lifetime.Singleton);
                builder.Register<IClientService<MasterProjectorPacket>, FakeMasterService>(Lifetime.Singleton);
            }
            else
            {
                builder.Register<IClientService<ProjectorPacket>, TCPClientBase<ProjectorPacket>>(Lifetime.Singleton);
                builder.Register<IClientService<MasterProjectorPacket>, TCPClientBase<MasterProjectorPacket>>(Lifetime.Singleton);
            }

            builder.Register<IMessageParser<ProjectorPacket>, ProjectorPacketParser>(Lifetime.Singleton);
            builder.Register<IMessageParser<MasterProjectorPacket>, MasterProjectorPacketParser>(Lifetime.Singleton);
            builder.Register<IPacketRouter, PacketRouter>(Lifetime.Singleton);
            builder.RegisterComponent(_uiService).As<IUIService>();
            builder.RegisterComponent(_dispatcher).As<IMainThreadDispatcher>();
            builder.Register<GameRuntime>(Lifetime.Singleton);

            builder.Register<ICollisionSolver, CollisionSolver>(Lifetime.Singleton);
            builder.Register<IPointCalculator, FakePointCalculator>(Lifetime.Singleton);
            builder.Register<ISensorProvider, SensorProvider>(Lifetime.Singleton);
            builder.Register<ITargetGenerator, TargetGenerator>(Lifetime.Singleton);

            builder.Register<NetworkBootstrap>(Lifetime.Singleton);
            builder.Register<GameBootstrap>(Lifetime.Singleton);
            builder.Register<RuntimeContext>(Lifetime.Singleton);

            builder.RegisterInstance(
                new UIRoot(_targetParent)
            );

            builder.RegisterEntryPoint<RootBootstrap>();

        }

        protected override void OnDestroy()
        {
            Debug.Log("Scope Destroy");
            base.OnDestroy();
        }
    }
}