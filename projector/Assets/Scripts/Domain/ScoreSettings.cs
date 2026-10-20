using System;
using UnityEngine.Serialization;

namespace Struckout.Domain
{
    /// <summary>
    /// 的に当たったときの点数。
    ///
    /// **点数はまだ確定していない。** 既定値は仮のもので、実機で調整する前提。
    /// 数字をコードに埋めずここに集めてあるのは、決まったときに
    /// Inspector だけで差し替えられるようにするため。
    ///
    /// 的の大きさは <see cref="Struckout.Application.ITargetGenerator"/> が
    /// 周囲の余白から決めるので、的ごとに変わる。小さい的ほど当てにくいので
    /// 高得点にしてある。
    ///
    /// 難易度で点数を変えるかはまだ決めていない。変えるなら
    /// <see cref="Struckout.Application.IPointCalculator"/> に難易度を渡すところから。
    /// </summary>
    [Serializable]
    public class ScoreSettings
    {
        /// <summary>
        /// 的の大きさごとの点数。
        ///
        /// 直径が <see cref="maxDiameter"/> 以下の段のうち、
        /// いちばん小さい段が選ばれる。Inspector で並べ替えても結果は変わらない。
        /// </summary>
        [Serializable]
        public class SizeTier
        {
            /// <summary>この段が受け持つ直径の上限。</summary>
            [FormerlySerializedAs("MaxDiameter")] public float maxDiameter;

            /// <summary>この段に当たったときの点数。</summary>
            [FormerlySerializedAs("Points")] public int points;
        }

        /// <summary>
        /// 仮の値。段を一つだけにすれば「当たれば一律で何点」にもできる。
        /// </summary>
        [FormerlySerializedAs("Tiers")] public SizeTier[] tiers =
        {
            new() { maxDiameter = 150f, points = 5 },
            new() { maxDiameter = 300f, points = 3 },
        };

        /// <summary>どの段にも収まらない大きな的の点数。</summary>
        [FormerlySerializedAs("DefaultPoints")] public int defaultPoints = 1;

        /// <summary>
        /// <paramref name="diameter"/> の的に当たったときの点数を返す。
        ///
        /// 得点は game_master に差分として送られ、プロトコル上は符号なしなので
        /// 負の値は返さない。
        /// </summary>
        public int PointsFor(float diameter)
        {
            int points = defaultPoints;

            if (tiers != null)
            {
                float best = float.PositiveInfinity;
                foreach (var tier in tiers)
                {
                    if (tier == null) continue;
                    if (diameter > tier.maxDiameter) continue;

                    // 収まる段のうち、いちばん狭い段を採る。
                    if (tier.maxDiameter < best)
                    {
                        best = tier.maxDiameter;
                        points = tier.points;
                    }
                }
            }

            return points < 0 ? 0 : points;
        }

        public override string ToString() =>
            $"tiers={(tiers == null ? 0 : tiers.Length)} default={defaultPoints}";
    }
}
