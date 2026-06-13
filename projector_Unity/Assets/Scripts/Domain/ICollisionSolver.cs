using System.Collections.Generic;
using Struckout.Dto.V1;

namespace Struckout.Domain
{
    public interface ICollisionSolver
    {
        bool IsCollision(CollisionPoint collisionPoint, List<Target> targets, out Target targetPoint);
    }
}