using Struckout.Application;
using Struckout.Domain;

namespace Struckout.Infrastructure
{
    public class FakePointCalculator : IPointCalculator
    {
        public int CalculatePoint(Target target)
        {
            return 1;
        }
    }
}