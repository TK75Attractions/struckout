using UnityEngine;
namespace StruckOut.Domain
{
    public interface ISensorProvider
    {
        CollisionPoint GetSensorData();
    }
}