using System;

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
        /// 直径が <see cref="SizeTier.MaxDiameter"/> 以下の段のうち、
        /// いちばん小さい段が選ばれる。Inspector で並べ替えても結果は変わらない。
        /// </summary>
        [Serializable]
        public class SizeTier
        {
            /// <summary>この段が受け持つ直径の上限。</summary>
            public float MaxDiameter;

            /// <summary>この段に当たったときの点数。</summary>
            public int Points;
        }

        /// <summary>
        /// 仮の値。段を一つだけにすれば「当たれば一律で何点」にもできる。
        /// </summary>
        public SizeTier[] Tiers =
        {
            new() { MaxDiameter = 150f, Points = 5 },
            new() { MaxDiameter = 300f, Points = 3 },
        };

        /// <summary>どの段にも収まらない大きな的の点数。</summary>
        public int DefaultPoints = 1;

        /// <summary>
        /// <paramref name="diameter"/> の的に当たったときの点数を返す。
        ///
        /// 得点は game_master に差分として送られ、プロトコル上は符号なしなので
        /// 負の値は返さない。
        /// </summary>
        public int PointsFor(float diameter)
        {
            int points = DefaultPoints;

            if (Tiers != null)
            {
                float best = float.PositiveInfinity;
                foreach (var tier in Tiers)
                {
                    if (tier == null) continue;
                    if (diameter > tier.MaxDiameter) continue;

                    // 収まる段のうち、いちばん狭い段を採る。
                    if (tier.MaxDiameter < best)
                    {
                        best = tier.MaxDiameter;
                        points = tier.Points;
                    }
                }
            }

            return points < 0 ? 0 : points;
        }

        public override string ToString() =>
            $"tiers={(Tiers == null ? 0 : Tiers.Length)} default={DefaultPoints}";
    }
}
