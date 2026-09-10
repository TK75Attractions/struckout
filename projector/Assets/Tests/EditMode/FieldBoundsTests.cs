using NUnit.Framework;
using Struckout.Domain;

namespace Struckout.Tests
{
    /// <summary>
    /// 盤面の矩形。的の配置と描画のスケールが同じ値を見ていることが前提なので、
    /// 既定値は CollisionCoordinateTransform が想定している 1920x1080 と揃っている必要がある。
    /// </summary>
    public class FieldBoundsTests
    {
        private const float Tolerance = 0.0001f;

        [Test]
        public void 既定は物理座標の変換係数と同じ盤面を指す()
        {
            var field = new FieldBounds();

            // CollisionCoordinateTransform の既定が x[-1,1]m -> 0〜1920 を前提にしている。
            Assert.That(field.Width, Is.EqualTo(1920f).Within(Tolerance));
            Assert.That(field.Height, Is.EqualTo(1080f).Within(Tolerance));
        }

        [Test]
        public void 中心は幅と高さの半分()
        {
            var field = new FieldBounds(800f, 600f);

            Assert.That(field.CentreX, Is.EqualTo(400f).Within(Tolerance));
            Assert.That(field.CentreY, Is.EqualTo(300f).Within(Tolerance));
        }

        [Test]
        public void 縦横比を返す()
        {
            Assert.That(new FieldBounds().Aspect, Is.EqualTo(1920f / 1080f).Within(Tolerance));
        }

        [Test]
        public void 高さが0以下でも比率で0除算しない()
        {
            Assert.That(new FieldBounds(100f, 0f).Aspect, Is.EqualTo(1f).Within(Tolerance));
        }

        [TestCase(0f, 0f, true, TestName = "左下は内側")]
        [TestCase(1920f, 1080f, true, TestName = "右上は内側")]
        [TestCase(960f, 540f, true, TestName = "中央は内側")]
        [TestCase(-1f, 540f, false, TestName = "左に出ている")]
        [TestCase(960f, 1081f, false, TestName = "上に出ている")]
        public void 盤面の内外を判定する(float x, float y, bool expected)
        {
            Assert.That(new FieldBounds().Contains(x, y), Is.EqualTo(expected));
        }
    }
}
