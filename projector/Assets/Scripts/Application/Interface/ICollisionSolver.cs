using System.Collections.Generic;
using Tk75Attractions.Struckout.V1;
using Struckout.Domain;

namespace Struckout.Application
{
    public interface ICollisionSolver
    {
        bool TryCollision(CollisionPoint collisionPoint, IReadOnlyList<Target> targets, out Target targetPoint);
    }
}