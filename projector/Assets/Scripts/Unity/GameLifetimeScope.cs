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
        [Tooltip("的とマーカーを置く親。Canvas ではなくワールド空間の Transform。")]
        private Transform _targetParent;

        [SerializeField]
        [Tooltip("盤面がちょうど収まるようにカメラを合わせる。")]
        private FieldCamera _fieldCamera;

        [SerializeField]
        [Tooltip("的を置ける盤面の広さ (px)。的の配置と描画のスケールがこれを共有する。")]
        private FieldBounds _field = new();

        [SerializeField]
        [Tooltip("ワールド 1 unit あたりの盤面ピクセル数。カメラの表示範囲はここから導かれる。")]
        private float _pixelsPerUnit = 100f;

        [SerializeField]
        [Tooltip("Fake にすると ball_tracker / game_master なしで起動できる。コマンドライン引数と環境変数で上書きできる。")]
        private NetworkSettings _networkSettings = new();

        [SerializeField]
        [Tooltip("ball_tracker から届く物理座標 (m) を的の描画座標に直す係数。実測して合わせること。")]
        private CollisionCoordinateTransform _collisionTransform = new();

        [SerializeField]
        [Tooltip("的の数とクールダウン時間。")]
        private GameSettings _gameSettings = new();

        [SerializeField]
        [Tooltip("的の大きさごとの点数。まだ仮の値なので、決まったらここで差し替える。")]
        private ScoreSettings _scoreSettings = new();

        protected override void Configure(IContainerBuilder builder)
        {
            var networkSettings = NetworkSettingsResolver.Resolve(_networkSettings);
            builder.RegisterInstance(networkSettings);

            // 物理座標 -> 描画座標 の係数。SensorProvider が使う。
            Debug.Log($"[Collision] transform {_collisionTransform}");
            builder.RegisterInstance(_collisionTransform);

            // 盤面の広さは的の配置 (TargetGenerator) と描画 (WorldCoordinateTransform) の
            // 両方が使う。片方だけ変えられないよう、同じインスタンスを配る。
            builder.RegisterInstance(_field);

            var world = new WorldCoordinateTransform(_field, _pixelsPerUnit);
            Debug.Log($"[Field] {world}");
            builder.RegisterInstance(world);

            Debug.Log($"[Game] {_gameSettings}");
            builder.RegisterInstance(_gameSettings);

            Debug.Log($"[Score] {_scoreSettings}");
            builder.RegisterInstance(_scoreSettings);

            if (networkSettings.Mode == NetworkMode.Fake)
            {
                builder.Register<IClientService<ProjectorPacket>, FakeClientService>(Lifetime.Singleton);
                builder.Register<IGameMasterClient, FakeGameMasterClient>(Lifetime.Singleton);
            }
            else
            {
                builder.Register<IClientService<ProjectorPacket>, TCPClientBase<ProjectorPacket>>(Lifetime.Singleton);
                builder.Register<IGameMasterClient, GrpcGameMasterClient>(Lifetime.Singleton);
            }

            builder.Register<IMessageParser<ProjectorPacket>, ProjectorPacketParser>(Lifetime.Singleton);
            builder.Register<IPacketRouter, PacketRouter>(Lifetime.Singleton);
            builder.RegisterComponent(_uiService).As<IUIService>();
            builder.RegisterComponent(_dispatcher).As<IMainThreadDispatcher>();
            if (_fieldCamera != null) builder.RegisterComponent(_fieldCamera);
            else Debug.LogWarning("FieldCamera is not assigned; the camera will not match the field.");
            builder.Register<GameRuntime>(Lifetime.Singleton);

            builder.Register<ICollisionSolver, CollisionSolver>(Lifetime.Singleton);
            builder.Register<IPointCalculator, PointCalculator>(Lifetime.Singleton);
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