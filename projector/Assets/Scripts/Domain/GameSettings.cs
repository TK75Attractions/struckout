using System;
using UnityEngine.Serialization;

namespace Struckout.Domain
{
    /// <summary>
    /// ゲーム進行の調整値。Inspector から変更できる。
    /// </summary>
    [Serializable]
    public class GameSettings
    {
        /// <summary>初期配置する的の数。</summary>
        [FormerlySerializedAs("InitialTargetCount")] public int initialTargetCount = 4;

        public override string ToString() => $"targets={initialTargetCount}";
    }
}
