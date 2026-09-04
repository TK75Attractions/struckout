namespace Struckout.Domain
{
    /// <summary>
    /// 1 回の着弾がどう処理されたか。
    /// マーカーの色を分けて、外したのかクールダウン中だったのかを見て区別できるようにする。
    /// </summary>
    public enum CollisionResult
    {
        /// <summary>どの的にも当たらなかった。</summary>
        Missed = 0,

        /// <summary>的に当たって得点した。</summary>
        Scored = 1,

        /// <summary>的には当たったが、その的がクールダウン中で得点にならなかった。</summary>
        CoolingDown = 2,
    }
}
