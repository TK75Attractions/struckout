using System.Collections.Generic;
using Struckout.Dto.V1;
using Struckout.Domain;

namespace Struckout.Application
{
    public interface ICollisionSolver
    {
        bool IsCollision(CollisionPoint collisionPoint, List<Target> targets, out Target targetPoint);
    }
}