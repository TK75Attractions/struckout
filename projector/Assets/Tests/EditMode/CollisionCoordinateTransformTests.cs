using NUnit.Framework;
using Struckout.Domain;

namespace Struckout.Tests
{
    /// <summary>
    /// 物理座標 (m) から描画座標への変換。
    /// 係数は盤面の実寸が決まるまで暫定なので、ここで固定しているのは
    /// 「暫定値がどう写るか」ではなく「式が正しいか」。
    /// </summary>
    public class CollisionCoordinateTransformTests
    {
        private const float Tolerance = 0.001f;

        /// <summary>既定値は物理 x[-1, 1] / y[0, 2] が 1920x1080 に収まる想定。</summary>
        [TestCase(0.0, 1.0, 960.0, 540.0, TestName = "既定係数: 中央")]
        [TestCase(-1.0, 0.0, 0.0, 0.0, TestName = "既定係数: 左下")]
        [TestCase(1.0, 2.0, 1920.0, 1080.0, TestName = "既定係数: 右上")]
        [TestCase(-1.0, 2.0, 0.0, 1080.0, TestName = "既定係数: 左上")]
        [TestCase(1.0, 0.0, 1920.0, 0.0, TestName = "既定係数: 右下")]
        public void 既定の係数で四隅と中央が画面に収まる(
            double physicalX, double physicalY, double expectedX, double expectedY)
        {
            var transform = new CollisionCoordinateTransform();

            var (x, y) = transform.ToRenderSpace(physicalX, physicalY);

            Assert.That(x, Is.EqualTo(expectedX).Within(Tolerance));
            Assert.That(y, Is.EqualTo(expectedY).Within(Tolerance));
        }

        [Test]
        public void 範囲外の座標もそのまま外挿される()
        {
            var transform = new CollisionCoordinateTransform();

            var (x, y) = transform.ToRenderSpace(-2.0, -1.0);

            // 判定側で弾く前提なので、変換は範囲で丸めない。
            Assert.That(x, Is.EqualTo(-960.0).Within(Tolerance));
            Assert.That(y, Is.EqualTo(-540.0).Within(Tolerance));
        }

        [Test]
        public void FlipX_は原点をまたいで左右を反転する()
        {
            var transform = new CollisionCoordinateTransform { FlipX = true };

            var (x, y) = transform.ToRenderSpace(1.0, 1.0);

            Assert.That(x, Is.EqualTo(0.0).Within(Tolerance), "x=+1 が左端に来る");
            Assert.That(y, Is.EqualTo(540.0).Within(Tolerance), "y は影響を受けない");
        }

        [Test]
        public void FlipY_は原点をまたいで上下を反転する()
        {
            var transform = new CollisionCoordinateTransform { FlipY = true };

            var (x, y) = transform.ToRenderSpace(0.0, 1.0);

            Assert.That(x, Is.EqualTo(960.0).Within(Tolerance), "x は影響を受けない");
            Assert.That(y, Is.EqualTo(-540.0).Within(Tolerance));
        }

        [Test]
        public void SwapAxes_は入力の縦横を入れ替えてから変換する()
        {
            var transform = new CollisionCoordinateTransform { SwapAxes = true };

            var (x, y) = transform.ToRenderSpace(2.0, 0.0);

            // (2, 0) -> 入れ替えて (0, 2) -> 中央上端
            Assert.That(x, Is.EqualTo(960.0).Within(Tolerance));
            Assert.That(y, Is.EqualTo(1080.0).Within(Tolerance));
        }

        [Test]
        public void SwapAxes_と_Flip_は併用できる()
        {
            var transform = new CollisionCoordinateTransform { SwapAxes = true, FlipX = true };

            var (x, _) = transform.ToRenderSpace(0.0, 1.0);

            // 入れ替えて (1, 0)、FlipX で x=+1 が左端
            Assert.That(x, Is.EqualTo(0.0).Within(Tolerance));
        }

        [Test]
        public void 係数と原点を変えれば任意の範囲に写せる()
        {
            // 盤面が実測で x[0, 1.6] m / y[0.5, 1.7] m だったとする想定。
            // px/m を縦横で揃えれば円が楕円にならない。
            var transform = new CollisionCoordinateTransform
            {
                PixelsPerMetreX = 900f,
                PixelsPerMetreY = 900f,
                OriginX = 0f,
                OriginY = -450f,
            };

            var (left, bottom) = transform.ToRenderSpace(0.0, 0.5);
            var (right, top) = transform.ToRenderSpace(1.6, 1.7);

            Assert.That(left, Is.EqualTo(0.0).Within(Tolerance));
            Assert.That(bottom, Is.EqualTo(0.0).Within(Tolerance));
            Assert.That(right, Is.EqualTo(1440.0).Within(Tolerance));
            Assert.That(top, Is.EqualTo(1080.0).Within(Tolerance));
        }
    }
}
