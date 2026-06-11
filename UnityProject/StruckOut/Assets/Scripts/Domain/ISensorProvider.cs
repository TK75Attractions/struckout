using UnityEngine;
using Struckout.Dto.V1;

namespace Struckout.Domain
{
    public interface ISensorProvider
    {
        CollisionPoint GetSensorData();
    }
}