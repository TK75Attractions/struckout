using Struckout.Domain.TargetDefinition;

namespace Struckout.Application.Interface
{
    public interface IPointCalculator
    {
        int CalculatePoint(Target target);
    }
}