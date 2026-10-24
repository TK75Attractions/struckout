using System;
using UnityEngine.Serialization;

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
        [FormerlySerializedAs("Mode")] public NetworkMode mode = NetworkMode.Real;

        /// <summary>ball_tracker (collision の送り元)。</summary>
        [FormerlySerializedAs("TrackerHost")] public string trackerHost = "127.0.0.1";
        [FormerlySerializedAs("TrackerPort")] public int trackerPort = 5000;

        /// <summary>game_master。gRPC (h2c) でつなぐ。</summary>
        [FormerlySerializedAs("MasterHost")] public string masterHost = "127.0.0.1";
        [FormerlySerializedAs("MasterPort")] public int masterPort = 8020;

        /// <summary>
        /// この筐体の号機番号。的の登録・命中通知と ListenEvents に必要。
        /// ListenEvents は game_master 側がこの番号で絞り込むので、
        /// 間違っていると繋がってはいるのにイベントが一件も来ない。
        ///
        /// 採番する仕組みが game_master に無い (machines テーブルも無い) ため、
        /// 各筐体で人間が設定する。誰がどう決めるかは未決。
        /// </summary>
        [FormerlySerializedAs("MachineId")] public int machineId = 1;

        /// <summary>接続を試す回数。1 なら再試行しない。失敗するたびに指数バックオフで待つ。</summary>
        [FormerlySerializedAs("ConnectAttempts")] public int connectAttempts = 5;

        public NetworkSettings Clone() => new()
        {
            mode = mode,
            trackerHost = trackerHost,
            trackerPort = trackerPort,
            masterHost = masterHost,
            masterPort = masterPort,
            machineId = machineId,
            connectAttempts = connectAttempts,
        };

        public override string ToString() =>
            $"mode={mode} tracker={trackerHost}:{trackerPort} master={masterHost}:{masterPort} machine={machineId} attempts={connectAttempts}";
    }
}
