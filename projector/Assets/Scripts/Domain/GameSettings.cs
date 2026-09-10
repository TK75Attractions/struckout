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

        public override string ToString() => $"targets={InitialTargetCount}";
    }
}
