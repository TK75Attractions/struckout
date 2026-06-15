using Cysharp.Threading.Tasks;

namespace Struckout.Bootstrap
{
    internal interface IAsyncDestroy
    {
        UniTask OnDestroy();
    }
}