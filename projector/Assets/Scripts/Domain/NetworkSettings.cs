using System;

namespace Struckout.Domain
{
    /// <summary>
    /// 接続先と、実機かダミーかの切り替え。
    ///
    /// 既定のポートは api/spec/servers.yaml に合わせている
    /// (tracker.projector = 5000、game-master.grpc = 8020)。
    ///
    /// Inspector で編集できるほか、コマンドライン引数と環境変数で上書きできる。
    /// 詳しくは <see cref="Struckout.Infrastructure.NetworkSettingsResolver"/> を参照。
    /// </summary>
    [Serializable]
    public class NetworkSettings
    {
        public NetworkMode Mode = NetworkMode.Real;

        /// <summary>ball_tracker (collision の送り元)。</summary>
        public string TrackerHost = "127.0.0.1";
        public int TrackerPort = 5000;

        /// <summary>game_master。gRPC (h2c) でつなぐ。</summary>
        public string MasterHost = "127.0.0.1";
        public int MasterPort = 8020;

        /// <summary>
        /// この筐体の号機番号。AddScore と ListenEvents の両方に必要。
        /// ListenEvents は game_master 側がこの番号で絞り込むので、
        /// 間違っていると繋がってはいるのにイベントが一件も来ない。
        ///
        /// 採番する仕組みが game_master に無い (machines テーブルも無い) ため、
        /// 各筐体で人間が設定する。誰がどう決めるかは未決。
        /// </summary>
        public int MachineId = 1;

        /// <summary>接続を試す回数。1 なら再試行しない。失敗するたびに指数バックオフで待つ。</summary>
        public int ConnectAttempts = 5;

        public NetworkSettings Clone() => new()
        {
            Mode = Mode,
            TrackerHost = TrackerHost,
            TrackerPort = TrackerPort,
            MasterHost = MasterHost,
            MasterPort = MasterPort,
            MachineId = MachineId,
            ConnectAttempts = ConnectAttempts,
        };

        public override string ToString() =>
            $"mode={Mode} tracker={TrackerHost}:{TrackerPort} master={MasterHost}:{MasterPort} machine={MachineId} attempts={ConnectAttempts}";
    }
}
