using Struckout.Dto.V1;

namespace Struckout.Application
{
    public interface ISensorProvider
    {
        CollisionPoint GetSensorData();
    }
}