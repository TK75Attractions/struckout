namespace Struckout.Domain
{
    public enum NetworkMode
    {
        /// <summary>実際に ball_tracker / game_master に TCP で接続する。</summary>
        Real = 0,

        /// <summary>
        /// 対向を一切必要とせず、ダミーのパケットを自分で流す。
        /// 描画やアニメーションだけを触りたいときに使う。
        /// </summary>
        Fake = 1,
    }
}
