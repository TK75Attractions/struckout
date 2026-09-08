using System.Collections.Generic;
using System.Text;
using Struckout.Domain;
using Tk75Attractions.Struckout.V1;
using System;
using UnityEngine;

namespace Struckout.Application
{
    public class CollisionSolver : ICollisionSolver
    {
        private readonly CollisionCoordinateTransform _transform;

        public CollisionSolver(CollisionCoordinateTransform transform)
        {
            _transform = transform;
        }

        public bool TryCollision(CollisionPoint collisionPoint, IReadOnlyList<Target> targets, out Target target)
        {
            target = null;
            foreach (var tar in targets)
            {
                if(IsWithinTarget(collisionPoint, tar))
                {
                    target = tar;
                    break;
                }
            }

            if (_transform != null && _transform.LogConversions)
            {
                LogDecision(collisionPoint, targets, target);
            }

            return target != null;
        }

        bool IsWithinTarget(CollisionPoint collisionPoint, Target target)
        {
            switch (target.Type)
            {
                case TargetType.Circle:
                    {
                        var targetPoint = target.Coordinate;
                        var distance = Math.Pow(collisionPoint.X - targetPoint.X, 2) + Math.Pow(collisionPoint.Y - targetPoint.Y, 2);
                        return distance <= target.RadiusSquared;
                    }
                default:
                    throw new Exception($"Unsupported target type {target.Type}");
            }
        }

        /// <summary>
        /// 当たり判定の内訳をそのまま出す。
        /// 画面に見えている円と、状態が持っている的が食い違っていないかを確認するためのもの。
        /// 的の数がここに出る数と画面上の円の数が違えば、UI と状態がずれている。
        /// </summary>
        private void LogDecision(CollisionPoint point, IReadOnlyList<Target> targets, Target hit)
        {
            var sb = new StringBuilder();
            sb.Append($"[Hit] point=({point.X:F1}, {point.Y:F1}) targets={targets.Count} -> ");
            sb.AppendLine(hit == null ? "MISS" : $"HIT at ({hit.Coordinate.X:F1}, {hit.Coordinate.Y:F1})");

            for (int i = 0; i < targets.Count; i++)
            {
                var t = targets[i];
                double dx = point.X - t.Coordinate.X;
                double dy = point.Y - t.Coordinate.Y;
                double distance = Math.Sqrt(dx * dx + dy * dy);

                sb.AppendLine(
                    $"    [{i}] centre=({t.Coordinate.X:F1}, {t.Coordinate.Y:F1}) " +
                    $"radius={t.Radius:F1} diameter={t.Diameter:F1} " +
                    $"distance={distance:F1} {(distance <= t.Radius ? "INSIDE" : "outside")}");
            }

            Debug.Log(sb.ToString());
        }
    }
}
