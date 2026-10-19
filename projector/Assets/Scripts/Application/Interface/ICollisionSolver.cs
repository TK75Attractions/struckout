using System.Collections.Generic;
using Struckout.Domain.TargetDefinition;
using Tk75Attractions.Struckout.V1;

namespace Struckout.Application.Interface
{
    public interface ICollisionSolver
    {
        bool TryCollision(CollisionPoint collisionPoint, IReadOnlyList<Target> targets, out Target targetPoint);
    }
}