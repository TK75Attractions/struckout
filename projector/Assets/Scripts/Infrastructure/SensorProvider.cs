using System;
using Struckout.Application;
using Struckout.Domain;
using Tk75Attractions.Struckout.V1;
using UnityEngine;

namespace Struckout.Infrastructure
{
    /// <summary>
    /// ball_watcher から届く物理座標 (m) の衝突点を、描画座標に直してから先へ流す。
    ///
    /// PacketRouter と GameRuntime の間にあるこの層が、
    /// 座標系が変わる唯一の場所になるようにしている。
    /// </summary>
    public class SensorProvider : ISensorProvider
    {
        private readonly CollisionCoordinateTransform _transform;

        public event Action<CollisionPoint> OnCollisionReceived;

        public SensorProvider(CollisionCoordinateTransform transform)
        {
            _transform = transform ?? throw new ArgumentNullException(nameof(transform));
        }

        public void GetSensorData(CollisionPoint point)
        {
            if (point == null) return;

            var (x, y) = _transform.ToRenderSpace(point.X, point.Y);

            if (_transform.LogConversions)
            {
                Debug.Log($"[Collision] ({point.X:F3}, {point.Y:F3}) m -> ({x:F1}, {y:F1})");
            }

            OnCollisionReceived?.Invoke(new CollisionPoint { X = x, Y = y });
        }
    }
}
