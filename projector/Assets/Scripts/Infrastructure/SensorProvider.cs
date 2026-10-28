using System;
using JetBrains.Annotations;
using Struckout.Application.Interface;
using Struckout.Domain;
using Tk75Attractions.Struckout.V1;
using Microsoft.Extensions.Logging;

namespace Struckout.Infrastructure
{
    /// <summary>
    /// ball_tracker から届く物理座標 (m) の衝突点を、描画座標に直してから先へ流す。
    ///
    /// PacketRouter と GameRuntime の間にあるこの層が、
    /// 座標系が変わる唯一の場所になるようにしている。
    /// </summary>
    public class SensorProvider : ISensorProvider
    {
        private readonly ILogger<SensorProvider> _logger;

        private readonly CollisionCoordinateTransform _transform;

        public event Action<CollisionPoint> OnCollisionReceived;

        [UsedImplicitly(ImplicitUseKindFlags.InstantiatedNoFixedConstructorSignature)]
        public SensorProvider(CollisionCoordinateTransform transform, ILogger<SensorProvider> logger)
        {
            _logger = logger ?? throw new ArgumentNullException(nameof(logger));
            _transform = transform ?? throw new ArgumentNullException(nameof(transform));
        }

        public void GetSensorData(CollisionPoint point)
        {
            if (point == null) throw new ArgumentNullException(nameof(point));

            var (x, y) = _transform.ToRenderSpace(point.X, point.Y);

            if (_transform.logConversions)
            {
                _logger.LogInformation("[Collision] ({PointX:F3}, {PointY:F3}) m -> ({X:F1}, {Y:F1})", point.X, point.Y, x, y);
            }

            OnCollisionReceived?.Invoke(new CollisionPoint { X = x, Y = y });
        }
    }
}
