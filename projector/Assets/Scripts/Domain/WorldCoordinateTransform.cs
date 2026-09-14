using System;

namespace Struckout.Domain
{
    /// <summary>
    /// 盤面の描画座標 (px) を、Unity のワールド座標 (unit) に変換する。
    ///
    /// 座標の流れは以下のとおり。
    ///
    ///   物理 (m) --CollisionCoordinateTransform--> 盤面 (px) --ここ--> ワールド (unit)
    ///
    /// 盤面の原点は左下、ワールドの原点は盤面の中心。
    ///
    /// <see cref="PixelsPerUnit"/> はカメラの orthographicSize と必ず対応していなければ
    /// ならないが、両方を手で設定すると必ずいつかずれる。そのため
    /// <see cref="OrthographicSizeFor"/> でカメラ側の値をここから導き、
    /// FieldCamera が実行時に適用する。Inspector でカメラを直接いじる必要はない。
    /// </summary>
    [Serializable]
    public class WorldCoordinateTransform
    {
        /// <summary>ワールド 1 unit あたりの盤面ピクセル数。</summary>
        public float PixelsPerUnit = 100f;

        private readonly FieldBounds _field;

        public WorldCoordinateTransform() : this(new FieldBounds(), 100f) { }

        public WorldCoordinateTransform(FieldBounds field, float pixelsPerUnit = 100f)
        {
            _field = field ?? new FieldBounds();
            PixelsPerUnit = pixelsPerUnit > 0f ? pixelsPerUnit : 100f;
        }

        public FieldBounds Field => _field;

        /// <summary>盤面 X (0 が左端) → ワールド X (0 が中心)。</summary>
        public float ToWorldX(float pixelX) => (pixelX - _field.CentreX) / PixelsPerUnit;

        /// <summary>盤面 Y (0 が下端) → ワールド Y (0 が中心)。Y は上向きのまま。</summary>
        public float ToWorldY(float pixelY) => (pixelY - _field.CentreY) / PixelsPerUnit;

        /// <summary>長さの変換。原点の移動を伴わないので割るだけ。</summary>
        public float ToWorldLength(float pixels) => pixels / PixelsPerUnit;

        /// <summary>ワールド X → 盤面 X。ログで突き合わせるとき用。</summary>
        public float ToPixelX(float worldX) => worldX * PixelsPerUnit + _field.CentreX;

        /// <summary>ワールド Y → 盤面 Y。</summary>
        public float ToPixelY(float worldY) => worldY * PixelsPerUnit + _field.CentreY;

        /// <summary>
        /// 盤面全体が映るのに必要な orthographicSize (= 表示範囲の高さの半分)。
        ///
        /// 投影面の比率が盤面と違っても、的が画面外に出ないほうを選ぶ。
        /// 投影面が横長なら左右に、縦長なら上下に余白が出る。
        /// 従来の CanvasScaler は幅基準で、縦長の投影面では的が切れていた。
        /// </summary>
        /// <param name="viewportAspect">投影面の 幅 / 高さ。</param>
        public float OrthographicSizeFor(float viewportAspect)
        {
            float halfHeight = _field.Height / (2f * PixelsPerUnit);
            if (viewportAspect <= 0f) return halfHeight;

            float halfWidth = _field.Width / (2f * PixelsPerUnit);

            // 縦に収めるのに要る高さと、横に収めるのに要る高さの、大きいほう。
            return MathF.Max(halfHeight, halfWidth / viewportAspect);
        }

        public override string ToString() => $"{_field} ppu={PixelsPerUnit}";
    }
}
