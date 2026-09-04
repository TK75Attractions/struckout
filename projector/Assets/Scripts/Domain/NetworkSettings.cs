using System;

namespace Struckout.Domain
{
    /// <summary>
    /// 接続先と、実機かダミーかの切り替え。
    ///
    /// 既定のポートは api/spec/tracker_projector.yaml (5000) と
    /// api/spec/master_projector.yaml (5001) に合わせている。
    ///
    /// Inspector で編集できるほか、コマンドライン引数と環境変数で上書きできる。
    /// 詳しくは <see cref="Struckout.Infrastructure.NetworkSettingsResolver"/> を参照。
    /// </summary>
    [Serializable]
    public class NetworkSettings
    {
        public NetworkMode Mode = NetworkMode.Real;

        /// <summary>ball_watcher (collision の送り元)。</summary>
        public string TrackerHost = "127.0.0.1";
        public int TrackerPort = 5000;

        /// <summary>game_master (タッチパネル側)。</summary>
        public string MasterHost = "127.0.0.1";
        public int MasterPort = 5001;

        /// <summary>接続を試す回数。1 なら再試行しない。失敗するたびに指数バックオフで待つ。</summary>
        public int ConnectAttempts = 5;

        public NetworkSettings Clone() => new()
        {
            Mode = Mode,
            TrackerHost = TrackerHost,
            TrackerPort = TrackerPort,
            MasterHost = MasterHost,
            MasterPort = MasterPort,
            ConnectAttempts = ConnectAttempts,
        };

        public override string ToString() =>
            $"mode={Mode} tracker={TrackerHost}:{TrackerPort} master={MasterHost}:{MasterPort} attempts={ConnectAttempts}";
    }
}
