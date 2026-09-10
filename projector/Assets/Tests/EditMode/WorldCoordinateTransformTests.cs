using NUnit.Framework;
using Struckout.Domain;

namespace Struckout.Tests
{
    /// <summary>
    /// 盤面の描画座標 (px) からワールド座標 (unit) への変換。
    ///
    /// 描画クラスは MonoBehaviour で EditMode から触れないが、
    /// 「どこに、どの大きさで置くか」の計算だけはここに切り出してあるので検証できる。
    /// 見た目と当たり判定が一致するかは、結局この式が正しいかで決まる。
    /// </summary>
    public class WorldCoordinateTransformTests
    {
        private const float Tolerance = 0.0001f;

        /// <summary>既定は 1920x1080 の盤面を 100 px/unit で見る。</summary>
        [TestCase(960f, 540f, 0f, 0f, TestName = "中心はワールド原点")]
        [TestCase(0f, 0f, -9.6f, -5.4f, TestName = "左下")]
        [TestCase(1920f, 1080f, 9.6f, 5.4f, TestName = "右上")]
        [TestCase(0f, 1080f, -9.6f, 5.4f, TestName = "左上")]
        [TestCase(1920f, 0f, 9.6f, -5.4f, TestName = "右下")]
        public void 盤面の四隅と中央がワールド座標に写る(
            float pixelX, float pixelY, float expectedX, float expectedY)
        {
            var world = new WorldCoordinateTransform();

            Assert.That(world.ToWorldX(pixelX), Is.EqualTo(expectedX).Within(Tolerance));
            Assert.That(world.ToWorldY(pixelY), Is.EqualTo(expectedY).Within(Tolerance));
        }

        [Test]
        public void 盤面のYは上向きのまま反転しない()
        {
            var world = new WorldCoordinateTransform();

            // 盤面の Y が増えると、ワールドの Y も増える。
            Assert.That(world.ToWorldY(1080f), Is.GreaterThan(world.ToWorldY(0f)));
        }

        [Test]
        public void 長さは原点の移動を伴わずに割られる()
        {
            var world = new WorldCoordinateTransform();

            // 直径 500px の的はワールドで 5 unit。
            Assert.That(world.ToWorldLength(500f), Is.EqualTo(5f).Within(Tolerance));
            Assert.That(world.ToWorldLength(0f), Is.EqualTo(0f).Within(Tolerance));
        }

        [TestCase(0f, 0f)]
        [TestCase(1920f, 1080f)]
        [TestCase(123.5f, 987.25f)]
        public void ワールドへ移して戻すと元の盤面座標に戻る(float pixelX, float pixelY)
        {
            var world = new WorldCoordinateTransform();

            Assert.That(world.ToPixelX(world.ToWorldX(pixelX)), Is.EqualTo(pixelX).Within(Tolerance));
            Assert.That(world.ToPixelY(world.ToWorldY(pixelY)), Is.EqualTo(pixelY).Within(Tolerance));
        }

        [Test]
        public void 投影面が盤面と同じ比率なら縦がちょうど収まる()
        {
            var world = new WorldCoordinateTransform();

            // 1080px / 2 / 100 = 5.4
            Assert.That(world.OrthographicSizeFor(1920f / 1080f), Is.EqualTo(5.4f).Within(Tolerance));
        }

        [Test]
        public void 投影面が盤面より縦長なら上下に余白を作って全体を映す()
        {
            var world = new WorldCoordinateTransform();

            // 4:3 では横を収めるほうが厳しい。960px / (4/3) / 100 = 7.2
            float size = world.OrthographicSizeFor(4f / 3f);

            Assert.That(size, Is.EqualTo(7.2f).Within(Tolerance));

            // 幅がちょうど収まっている = 的が左右で切れない。
            Assert.That(size * (4f / 3f), Is.EqualTo(9.6f).Within(Tolerance));
        }

        [Test]
        public void 投影面が盤面より横長なら縦基準のまま左右に余白が出る()
        {
            var world = new WorldCoordinateTransform();

            float size = world.OrthographicSizeFor(21f / 9f);

            Assert.That(size, Is.EqualTo(5.4f).Within(Tolerance));

            // 横は盤面より広く映る。
            Assert.That(size * (21f / 9f), Is.GreaterThan(9.6f));
        }

        [Test]
        public void どの比率でも盤面全体が表示範囲に収まる()
        {
            var world = new WorldCoordinateTransform();

            foreach (float aspect in new[] { 1f, 4f / 3f, 16f / 10f, 16f / 9f, 21f / 9f, 32f / 9f })
            {
                float size = world.OrthographicSizeFor(aspect);

                Assert.That(size, Is.GreaterThanOrEqualTo(5.4f - Tolerance), $"aspect={aspect}");
                Assert.That(size * aspect, Is.GreaterThanOrEqualTo(9.6f - Tolerance), $"aspect={aspect}");
            }
        }

        [Test]
        public void 比率が不正なら縦基準にたおれる()
        {
            var world = new WorldCoordinateTransform();

            Assert.That(world.OrthographicSizeFor(0f), Is.EqualTo(5.4f).Within(Tolerance));
            Assert.That(world.OrthographicSizeFor(-1f), Is.EqualTo(5.4f).Within(Tolerance));
        }

        [Test]
        public void ピクセル密度を変えると表示範囲もついてくる()
        {
            // 200 px/unit なら同じ盤面が半分のワールドサイズになる。
            var world = new WorldCoordinateTransform(new FieldBounds(), 200f);

            Assert.That(world.ToWorldLength(500f), Is.EqualTo(2.5f).Within(Tolerance));
            Assert.That(world.OrthographicSizeFor(1920f / 1080f), Is.EqualTo(2.7f).Within(Tolerance));
        }

        [Test]
        public void ピクセル密度が0以下なら既定に戻す()
        {
            // 0 を通すと以降すべてが 0 除算になるので、ここで止める。
            var world = new WorldCoordinateTransform(new FieldBounds(), 0f);

            Assert.That(world.PixelsPerUnit, Is.EqualTo(100f).Within(Tolerance));
        }

        [Test]
        public void 盤面の比率を変えると変換もついてくる()
        {
            var world = new WorldCoordinateTransform(new FieldBounds(1280f, 1280f));

            Assert.That(world.ToWorldX(640f), Is.EqualTo(0f).Within(Tolerance));
            Assert.That(world.ToWorldY(0f), Is.EqualTo(-6.4f).Within(Tolerance));
            Assert.That(world.OrthographicSizeFor(1f), Is.EqualTo(6.4f).Within(Tolerance));
        }
    }
}
