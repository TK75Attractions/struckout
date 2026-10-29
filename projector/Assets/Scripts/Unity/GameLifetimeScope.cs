using Sentry;
using Sentry.Extensibility;
using Microsoft.Extensions.Logging;
using Struckout.Application;
using Struckout.Application.Interface;
using Struckout.Domain;
using Struckout.Domain.Targets;
using Struckout.Infrastructure;
using Struckout.Bootstrap;
using Struckout.Infrastructure.Fake;
using Struckout.Infrastructure.Grpc;
using Struckout.Infrastructure.TCP;
using Struckout.Unity.Logging;
using UnityEngine;
using VContainer;
using VContainer.Unity;
using Tk75Attractions.Struckout.V1;
using UnityEngine.Serialization;

namespace Struckout.Unity
{
    public class GameLifetimeScope : LifetimeScope
    {
        private ILogger<GameLifetimeScope> _logger;

        [FormerlySerializedAs("_uiService")] [SerializeField]
        private UIService uiService;
        [FormerlySerializedAs("_dispatcher")] [SerializeField]
        private MainThreadDispatcher dispatcher;

        [FormerlySerializedAs("_targetParent")]
        [SerializeField]
        [Tooltip("的とマーカーを置く親。Canvas ではなくワールド空間の Transform。")]
        private Transform targetParent;

        [FormerlySerializedAs("_fieldCamera")]
        [SerializeField]
        [Tooltip("盤面がちょうど収まるようにカメラを合わせる。")]
        private FieldCamera fieldCamera;

        [FormerlySerializedAs("_field")]
        [SerializeField]
        [Tooltip("的を置ける盤面の広さ (px)。的の配置と描画のスケールがこれを共有する。")]
        private FieldBounds field = new();

        [FormerlySerializedAs("_pixelsPerUnit")]
        [SerializeField]
        [Tooltip("ワールド 1 unit あたりの盤面ピクセル数。カメラの表示範囲はここから導かれる。")]
        private float pixelsPerUnit = 100f;

        [FormerlySerializedAs("_networkSettings")]
        [SerializeField]
        [Tooltip("Fake にすると ball_tracker / game_master なしで起動できる。コマンドライン引数と環境変数で上書きできる。")]
        private NetworkSettings networkSettings = new();

        [FormerlySerializedAs("_collisionTransform")]
        [SerializeField]
        [Tooltip("ball_tracker から届く物理座標 (m) を的の描画座標に直す係数。実測して合わせること。")]
        private CollisionCoordinateTransform collisionTransform = new();

        [FormerlySerializedAs("_gameSettings")]
        [SerializeField]
        [Tooltip("的の数とクールダウン時間。")]
        private GameSettings gameSettings = new();

        protected override void Configure(IContainerBuilder builder)
        {
            var hub = HubAdapter.Instance;
            var scopes = new LoggerExternalScopeProvider();
            _logger = new ProjectorLogger<GameLifetimeScope>(hub, scopes);
            // RegisterInstance leaves ownership of the existing SDK with Sentry Unity.
            builder.RegisterInstance<IHub>(hub);
            builder.RegisterInstance<IExternalScopeProvider>(scopes);
            builder.Register(typeof(ProjectorLogger<>), Lifetime.Singleton)
                .As(typeof(ILogger<>));
            var resolver = new NetworkSettingsResolver(new ProjectorLogger<NetworkSettingsResolver>(hub, scopes));
            var networkSettings = resolver.Resolve(this.networkSettings);
            builder.RegisterInstance(networkSettings);

            // 物理座標 -> 描画座標 の係数。SensorProvider が使う。
            // 書き出したプレイヤーでも変えられるよう、設定ファイルがあれば Inspector の値を上書きする。
            // 実行中は F2 の調整画面 (CollisionCalibrationPanel) で同じインスタンスを書き換え、ファイルに保存できる。
            var inspectorCollisionTransform = new CollisionCoordinateTransform();
            inspectorCollisionTransform.CopyFrom(collisionTransform);
            var collisionTransformPath = CollisionTransformStore.ResolvePath();
            if (CollisionTransformStore.TryLoadInto(collisionTransform, collisionTransformPath, out var collisionTransformError))
            {
                _logger.LogInformation("[Collision] loaded {CollisionTransformPath}", collisionTransformPath);
            }
            else if (collisionTransformError != null)
            {
                _logger.LogWarning("[Collision] could not read {Path}; using the Inspector values: {Error}", collisionTransformPath, collisionTransformError);
            }
            else
            {
                _logger.LogInformation("[Collision] {Path} not found; using the Inspector values", collisionTransformPath);
            }

            _logger.LogInformation("[Collision] transform {CollisionTransform}", collisionTransform);
            builder.RegisterInstance(collisionTransform);
            builder.RegisterBuildCallback(container =>
            {
                var panel = new GameObject("CollisionCalibrationPanel").AddComponent<CollisionCalibrationPanel>();
                panel.transform.SetParent(transform, false);
                panel.Construct(
                    container.Resolve<CollisionCoordinateTransform>(),
                    inspectorCollisionTransform,
                    container.Resolve<IUIService>(),
                    collisionTransformPath, container.Resolve<ILogger<CollisionCalibrationPanel>>());
            });

            // 盤面の広さは的の配置 (TargetGenerator) と描画 (WorldCoordinateTransform) の
            // 両方が使う。片方だけ変えられないよう、同じインスタンスを配る。
            builder.RegisterInstance(field);

            var world = new WorldCoordinateTransform(field, pixelsPerUnit);
            _logger.LogInformation("[Field] {World}", world);
            builder.RegisterInstance(world);

            _logger.LogInformation("[Game] {GameSettings}", gameSettings);
            builder.RegisterInstance(gameSettings);

            if (networkSettings.mode == NetworkMode.Fake)
            {
                builder.Register<IClientService<ProjectorPacket>, FakeClientService>(Lifetime.Singleton);
                // Select the runtime constructor; the other constructor accepts test durations.
                builder.Register<IGameMasterClient>(container => new FakeGameMasterClient(
                    container.Resolve<ILogger<FakeGameMasterClient>>()), Lifetime.Singleton);
            }
            else
            {
                builder.Register<IClientService<ProjectorPacket>, TcpClientBase<ProjectorPacket>>(Lifetime.Singleton);
                builder.Register<IGameMasterClient, GrpcGameMasterClient>(Lifetime.Singleton);
            }

            builder.Register<IMessageParser<ProjectorPacket>, ProjectorPacketParser>(Lifetime.Singleton);
            builder.Register<IPacketRouter, PacketRouter>(Lifetime.Singleton);
            builder.RegisterComponent(uiService).As<IUIService>();
            builder.RegisterComponent(dispatcher).As<IMainThreadDispatcher>();
            if (fieldCamera != null) builder.RegisterComponent(fieldCamera);
            else _logger.LogWarning("FieldCamera is not assigned; the camera will not match the field.");
            builder.Register<GameRuntime>(Lifetime.Singleton);
            builder.Register<ScreenController>(Lifetime.Singleton);

            builder.Register<ICollisionSolver, CollisionSolver>(Lifetime.Singleton);
            builder.Register<ISensorProvider, SensorProvider>(Lifetime.Singleton);
            builder.Register<TargetGenerator>(Lifetime.Singleton);

            builder.Register<NetworkBootstrap>(Lifetime.Singleton);
            builder.Register<GameBootstrap>(Lifetime.Singleton);
            builder.Register<RuntimeContext>(Lifetime.Singleton);

            builder.RegisterInstance(
                new UIRoot(targetParent)
            );

            builder.RegisterEntryPoint<RootBootstrap>();

        }

        protected override void OnDestroy()
        {
            _logger?.LogInformation("Scope Destroy");
            base.OnDestroy();
        }
    }
}