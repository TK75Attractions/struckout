using System;
using NUnit.Framework;
using Struckout.Domain;

namespace Struckout.Tests
{
    public class ConnectionStateMachineTests
    {
        [TestCase(ConnectionState.Disconnected, ConnectionState.Connecting)]
        [TestCase(ConnectionState.Connecting, ConnectionState.Connected)]
        [TestCase(ConnectionState.Connecting, ConnectionState.Failed)]
        [TestCase(ConnectionState.Connected, ConnectionState.Disconnecting)]
        [TestCase(ConnectionState.Disconnecting, ConnectionState.Disconnected)]
        [TestCase(ConnectionState.Disconnecting, ConnectionState.Failed)]
        [TestCase(ConnectionState.Failed, ConnectionState.Connecting)]
        [TestCase(ConnectionState.Failed, ConnectionState.Disconnecting)]
        public void 許可された遷移は通る(ConnectionState from, ConnectionState to)
        {
            Assert.That(ConnectionStateMachine.Transition(from, to), Is.EqualTo(to));
        }

        /// <summary>
        /// 通信中に相手都合で切れたときに使う。これが無いと切断を状態に反映できず、
        /// 切れているのに Connected のままになる。
        /// </summary>
        [Test]
        public void 接続中から失敗へ遷移できる()
        {
            Assert.That(
                ConnectionStateMachine.Transition(ConnectionState.Connected, ConnectionState.Failed),
                Is.EqualTo(ConnectionState.Failed));
        }

        [TestCase(ConnectionState.Disconnected, ConnectionState.Connected, TestName = "接続処理を飛ばせない")]
        [TestCase(ConnectionState.Disconnected, ConnectionState.Failed, TestName = "切断済みから失敗にはならない")]
        [TestCase(ConnectionState.Connected, ConnectionState.Connecting, TestName = "接続済みから接続中には戻らない")]
        [TestCase(ConnectionState.Connecting, ConnectionState.Disconnecting, TestName = "接続中から切断処理には入らない")]
        [TestCase(ConnectionState.Failed, ConnectionState.Connected, TestName = "失敗から接続済みには飛べない")]
        public void 許可されていない遷移は例外になる(ConnectionState from, ConnectionState to)
        {
            Assert.Throws<ArgumentException>(() => ConnectionStateMachine.Transition(from, to));
        }
    }
}
