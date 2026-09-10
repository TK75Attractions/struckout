using System;

namespace Struckout.Domain
{
    /// <summary>
    /// ゲーム進行の調整値。Inspector から変更できる。
    /// </summary>
    [Serializable]
    public class GameSettings
    {
        /// <summary>初期配置する的の数。</summary>
        public int InitialTargetCount = 4;

        /// <summary>
        /// 的に当たってから再び当たるようになるまでの秒数。
        /// この間、的は色が変わり、当たっても得点にならない。
        /// </summary>
        public float TargetCooldownSeconds = 1.5f;

        public override string ToString() =>
            $"targets={InitialTargetCount} cooldown={TargetCooldownSeconds}s";
    }
}
