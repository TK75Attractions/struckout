using Cysharp.Threading.Tasks;

namespace Struckout.Bootstrap
{
    public interface IAsyncDestroy
    {
        UniTask DisposeAsync();
    }
}