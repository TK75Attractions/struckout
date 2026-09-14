namespace Struckout.Domain
{
    /// <summary>
    /// 1 回の着弾がどう処理されたか。
    /// マーカーの色を分けて、外したのかゲーム中でなかったのかを見て区別できるようにする。
    /// </summary>
    public enum CollisionResult
    {
        /// <summary>どの的にも当たらなかった。</summary>
        Missed = 0,

        /// <summary>的に当たって得点した。</summary>
        Scored = 1,

        /// <summary>ゲーム中ではないので判定しなかった。StartGame 待ち。</summary>
        Ignored = 2,
    }
}
