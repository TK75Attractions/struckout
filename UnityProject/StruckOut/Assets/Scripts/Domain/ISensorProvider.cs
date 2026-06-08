using UnityEngine;
using StruckOut.DTO;

namespace StruckOut.Domain
{
    public interface ISensorProvider
    {
        CollisionPoint GetSensorData();
    }
}