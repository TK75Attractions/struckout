using UnityEngine;
using Struckout.Dto.V1;

namespace StruckOut.Domain
{
    public interface ISensorProvider
    {
        CollisionPoint GetSensorData();
    }
}