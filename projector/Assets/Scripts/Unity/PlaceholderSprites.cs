using System;
using UnityEngine;

namespace Struckout.Unity
{
    /// <summary>
    /// 素材が用意されるまでの仮のスプライト。
    ///
    /// リポジトリに画像を1枚も置かずに動かすために、実行時に作っている。
    /// 絵ができたら Prefab の SpriteRenderer に割り当てればよく、
    /// そうすればここは呼ばれなくなる (<see cref="UIService"/> 参照)。
    ///
    /// 的は直径 500px 級で表示されるので、粗い画像だと縁が目立つ。
    /// 縁は符号付き距離から 1px 幅でアンチエイリアスしている。
    /// </summary>
    internal static class PlaceholderSprites
    {
        /// <summary>生成するテクスチャの一辺。pixelsPerUnit も同じ値にするので、大きさは 1 unit になる。</summary>
        private const int Resolution = 256;

        private static Sprite _circle;
        private static Sprite _diamond;

        /// <summary>的の仮の見た目。</summary>
        public static Sprite Circle =>
            _circle != null ? _circle : _circle = Build("PlaceholderCircle", (u, v) => 0.5f - Mathf.Sqrt(u * u + v * v));

        /// <summary>着弾マーカーの仮の見た目。的が円なので、区別できるよう菱形にしている。</summary>
        public static Sprite Diamond =>
            _diamond != null ? _diamond : _diamond = Build("PlaceholderDiamond", (u, v) => 0.5f - (Mathf.Abs(u) + Mathf.Abs(v)));

        /// <param name="signedDistance">
        /// テクスチャ内の位置 (u, v はどちらも -0.5〜0.5) を受け取り、
        /// 図形の内側で正、縁で 0、外側で負を返す。単位はテクスチャの一辺。
        /// </param>
        private static Sprite Build(string name, Func<float, float, float> signedDistance)
        {
            var texture = new Texture2D(Resolution, Resolution, TextureFormat.RGBA32, mipChain: false)
            {
                name = name,
                filterMode = FilterMode.Bilinear,
                wrapMode = TextureWrapMode.Clamp,
                // 実行時に作った使い捨てなので、シーンやアセットに紛れ込ませない。
                hideFlags = HideFlags.HideAndDontSave,
            };

            var pixels = new Color32[Resolution * Resolution];
            for (int y = 0; y < Resolution; y++)
            {
                // ピクセルの中心で評価する。
                float v = (y + 0.5f) / Resolution - 0.5f;
                for (int x = 0; x < Resolution; x++)
                {
                    float u = (x + 0.5f) / Resolution - 0.5f;

                    // 縁をまたぐ 1px ぶんで 0→1 に変える。
                    float coverage = Mathf.Clamp01(signedDistance(u, v) * Resolution + 0.5f);
                    pixels[y * Resolution + x] = new Color32(255, 255, 255, (byte)(coverage * 255f));
                }
            }

            texture.SetPixels32(pixels);
            texture.Apply(updateMipmaps: false);

            // pixelsPerUnit をテクスチャの一辺と同じにすると、スプライトはちょうど 1 unit 角になる。
            // CircleTargetUI は実際の bounds を見て拡縮するので、ここが 1 でなくても壊れはしない。
            var sprite = Sprite.Create(
                texture,
                new Rect(0f, 0f, Resolution, Resolution),
                new Vector2(0.5f, 0.5f),
                pixelsPerUnit: Resolution);
            sprite.name = name;
            sprite.hideFlags = HideFlags.HideAndDontSave;
            return sprite;
        }
    }
}
