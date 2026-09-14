using System;
using System.Threading.Tasks;
using Tk75Attractions.Struckout.V1;

namespace Struckout.Application
{
    public interface IClientService<T>
    {
        void RegisterPort(string host, int port);
        event Action<T> OnReceived;

        /// <summary>
        /// 接続が切れたときに発火する。自分から切ったときは呼ばれない。
        /// 受信スレッドから飛んでくるので、UI に触る購読側はマーシャリングすること。
        /// </summary>
        event Action ConnectionLost;
        Task<bool> ConnectAsync();

        /// <summary>失敗したら指数バックオフで <paramref name="maxAttempts"/> 回まで接続を試す。</summary>
        Task<bool> ConnectRetryAsync(int maxAttempts);

        Task DisconnectAsync();
    }
}