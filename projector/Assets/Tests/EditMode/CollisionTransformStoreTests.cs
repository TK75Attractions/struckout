using System;
using System.Collections.Generic;
using System.IO;
using NUnit.Framework;
using Struckout.Domain;
using Struckout.Infrastructure;

namespace Struckout.Tests
{
    /// <summary>
    /// 書き出したプレイヤーでも係数を変えられるようにする、設定ファイルの読み書き。
    /// </summary>
    public class CollisionTransformStoreTests
    {
        private string _directory;

        [SetUp]
        public void SetUp()
        {
            _directory = Path.Combine(Path.GetTempPath(), "struckout-tests-" + Guid.NewGuid().ToString("N"));
        }

        [TearDown]
        public void TearDown()
        {
            if (Directory.Exists(_directory)) Directory.Delete(_directory, true);
        }

        [Test]
        public void 場所はコマンドラインが環境変数より優先される()
        {
            var args = new Dictionary<string, string> { [CollisionTransformStore.ArgName] = "C:/a.json" };

            var path = CollisionTransformStore.ResolvePath(args, _ => "C:/b.json", "C:/default");

            Assert.That(path, Is.EqualTo("C:/a.json"));
        }

        [Test]
        public void コマンドラインが無ければ環境変数を使う()
        {
            var path = CollisionTransformStore.ResolvePath(
                new Dictionary<string, string>(),
                name => name == CollisionTransformStore.EnvName ? "C:/b.json" : null,
                "C:/default");

            Assert.That(path, Is.EqualTo("C:/b.json"));
        }

        [Test]
        public void どちらも無ければ既定のフォルダに置く()
        {
            var path = CollisionTransformStore.ResolvePath(new Dictionary<string, string>(), _ => null, "C:/default");

            Assert.That(path, Is.EqualTo(Path.Combine("C:/default", CollisionTransformStore.FileName)));
        }

        [Test]
        public void 保存して読み直すと同じ値になる()
        {
            var path = Path.Combine(_directory, "sub", CollisionTransformStore.FileName);
            var saved = new CollisionCoordinateTransform
            {
                PixelsPerMetreX = 905.5f, PixelsPerMetreY = 880.25f, OriginX = 410f, OriginY = -1350f,
                FlipX = true, SwapAxes = false, LogConversions = true,
            };

            CollisionTransformStore.Save(saved, path);
            var loaded = new CollisionCoordinateTransform();
            bool ok = CollisionTransformStore.TryLoadInto(loaded, path, out var error);

            Assert.That(ok, Is.True, error);
            Assert.That(loaded.ToString(), Is.EqualTo(saved.ToString()));
            Assert.That(loaded.LogConversions, Is.True);
        }

        [Test]
        public void ファイルが無ければ何も変えない()
        {
            var target = new CollisionCoordinateTransform { OriginX = 123f };

            bool ok = CollisionTransformStore.TryLoadInto(target, Path.Combine(_directory, "none.json"), out var error);

            Assert.That(ok, Is.False);
            Assert.That(error, Is.Null);
            Assert.That(target.OriginX, Is.EqualTo(123f));
        }

        [Test]
        public void 一メートルあたりが正でなければ読まずに理由を返す()
        {
            Directory.CreateDirectory(_directory);
            var path = Path.Combine(_directory, CollisionTransformStore.FileName);
            File.WriteAllText(path, "{ \"PixelsPerMetreX\": -900, \"OriginX\": 5 }");
            var target = new CollisionCoordinateTransform { OriginX = 123f };

            bool ok = CollisionTransformStore.TryLoadInto(target, path, out var error);

            Assert.That(ok, Is.False);
            Assert.That(error, Is.Not.Null);
            Assert.That(target.OriginX, Is.EqualTo(123f), "おかしな値なら Inspector の値のまま");
        }

        [Test]
        public void 書かれていない項目は既定値になる()
        {
            var loaded = CollisionTransformStore.FromJson("{ \"OriginX\": 5 }");

            Assert.That(loaded.OriginX, Is.EqualTo(5f));
            Assert.That(loaded.PixelsPerMetreX, Is.EqualTo(new CollisionCoordinateTransform().PixelsPerMetreX));
        }
    }
}
