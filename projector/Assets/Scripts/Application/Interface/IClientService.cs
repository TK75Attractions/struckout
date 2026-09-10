using System;
using System.Threading.Tasks;
using Google.Protobuf;
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

        /// <summary>
        /// 1 パケット送る。接続していなければ false。
        /// 受信型 <typeparamref name="T"/> と送信型は別なので、ここは IMessage で受ける
        /// (master は受信 MasterProjectorPacket / 送信 ProjectorMasterPacket)。
        /// </summary>
        Task<bool> SendAsync(IMessage packet);

        /// <summary>失敗したら指数バックオフで <paramref name="maxAttempts"/> 回まで接続を試す。</summary>
        Task<bool> ConnectRetryAsync(int maxAttempts);

        Task DisconnectAsync();
    }
}