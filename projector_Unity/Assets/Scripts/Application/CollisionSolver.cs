using System.Collections.Generic;
using Struckout.Domain;
using Struckout.Dto.V1;
using System;

namespace Struckout.Application
{
    public class CollisionSolver : ICollisionSolver
    {
        public bool IsCollision(CollisionPoint collisionPoint, List<Target> targets, out Target target)
        {
            target = null;
            foreach (var tar in targets)
            {
                if(IsWithinTarget(collisionPoint, tar))
                {
                    target = tar;
                    return true;
                }
            }
            return false;
        }

        bool IsWithinTarget(CollisionPoint collisionPoint, Target target)
        {
            switch (target.Type)
            {
                case TargetType.Circle:
                    {
                        var targetPoint = target.Coordinate;
                        var distance = Math.Sqrt(Math.Pow(collisionPoint.X - targetPoint.X, 2) + Math.Pow(collisionPoint.Y - targetPoint.Y, 2));
                        return distance <= target.Size;
                    }
                default:
                    throw new Exception($"Unsupported target type{target.Type}");
            }
        }
    }
}