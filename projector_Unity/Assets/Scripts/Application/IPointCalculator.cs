using Struckout.Domain;

namespace Struckout.Application
{
    public interface IPointCalculator
    {
        int CalculatePoint(Target target);
    }
}