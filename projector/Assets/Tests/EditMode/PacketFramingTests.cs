using System;
using System.Buffers.Binary;
using Google.Protobuf;
using NUnit.Framework;
using Tk75Attractions.Struckout.V1;

namespace Struckout.Tests
{
    /// <summary>
    /// TCP に載せるときのフレーミング。LE u32 の長さ + protobuf 本体。
    /// 相手側の実装は api/rust/src/lib.rs の write_packet / read_packet。
    ///
    /// 生成コードのズレで実際に壊れた箇所なので、
    /// 期待するバイト列を固定して回帰を検出できるようにしている。
    /// </summary>
    public class PacketFramingTests
    {
        /// <summary>ball_tracker が送ってくる衝突点。</summary>
        [Test]
        public void CollisionPoint_が既知のバイト列になる()
        {
            var packet = new ProjectorPacket
            {
                Point = new CollisionPoint { X = 123.5, Y = 456.25 },
            };

            var bytes = packet.ToByteArray();

            // 0A <len> 09 <x:double LE> 11 <y:double LE>
            Assert.That(BitConverter.ToString(bytes), Is.EqualTo(
                "0A-12-09-00-00-00-00-00-E0-5E-40-11-00-00-00-00-00-84-7C-40"));
        }

        /// <summary>game_master が送ってくる開始通知。</summary>
        [Test]
        public void StartGame_が既知のバイト列になる()
        {
            var packet = new MasterProjectorPacket
            {
                StartGame = new StartGame { Difficulty = Difficulty.Hard },
            };

            Assert.That(BitConverter.ToString(packet.ToByteArray()), Is.EqualTo("0A-02-08-02"));
        }

        /// <summary>projector が game_master に返す得点。</summary>
        [Test]
        public void 得点が既知のバイト列になる()
        {
            var packet = new ProjectorMasterPacket { Score = 7 };

            Assert.That(BitConverter.ToString(packet.ToByteArray()), Is.EqualTo("08-07"));
        }

        [Test]
        public void 長さヘッダはリトルエンディアン()
        {
            var header = new byte[4];
            BinaryPrimitives.WriteUInt32LittleEndian(header, 2000);

            Assert.That(header, Is.EqualTo(new byte[] { 208, 7, 0, 0 }),
                "api/rust 側のテストと同じ値");
        }

        [Test]
        public void フレームを組んで解くと元に戻る()
        {
            var original = new ProjectorPacket
            {
                Point = new CollisionPoint { X = -0.5, Y = 1.25 },
            };

            var payload = original.ToByteArray();
            var frame = new byte[4 + payload.Length];
            BinaryPrimitives.WriteUInt32LittleEndian(frame.AsSpan(0, 4), (uint)payload.Length);
            payload.CopyTo(frame, 4);

            uint length = BinaryPrimitives.ReadUInt32LittleEndian(frame.AsSpan(0, 4));
            Assert.That(length, Is.EqualTo((uint)payload.Length));

            var body = new byte[length];
            Array.Copy(frame, 4, body, 0, length);
            var decoded = ProjectorPacket.Parser.ParseFrom(body);

            Assert.That(decoded, Is.EqualTo(original));
            Assert.That(decoded.Point.X, Is.EqualTo(-0.5));
            Assert.That(decoded.Point.Y, Is.EqualTo(1.25));
        }

        /// <summary>
        /// ProjectorMasterPacket が生成コードに存在すること自体の確認。
        /// 以前 testTcpCLI 側の生成コードからこの型が丸ごと欠落していた。
        /// </summary>
        [Test]
        public void 送受信に使う型がすべて生成されている()
        {
            Assert.That(new ProjectorPacket(), Is.Not.Null);
            Assert.That(new MasterProjectorPacket(), Is.Not.Null);
            Assert.That(new ProjectorMasterPacket(), Is.Not.Null);
            Assert.That(new CollisionPoint(), Is.Not.Null);
            Assert.That(new StartGame(), Is.Not.Null);
        }
    }
}
