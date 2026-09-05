using System;
using System.Collections.Generic;

namespace Struckout.Domain
{
    public static class ConnectionStateMachine
    {
        private static readonly Dictionary<ConnectionState, HashSet<ConnectionState>> _transitions =
        new()
        {
            // Connected -> Failed は「通信中に切れた」。
            // これが無いと受信ループがエラーで抜けたことを状態に反映できない。
            { ConnectionState.Connected, new(){ ConnectionState.Disconnecting, ConnectionState.Failed } },
            { ConnectionState.Connecting, new(){ ConnectionState.Connected, ConnectionState.Failed } },
            { ConnectionState.Disconnected, new(){ ConnectionState.Connecting } },
            { ConnectionState.Disconnecting, new(){ ConnectionState.Disconnected, ConnectionState.Failed } },
            { ConnectionState.Failed, new(){ ConnectionState.Connecting, ConnectionState.Disconnecting } }
        };

        public static ConnectionState Transition(ConnectionState from, ConnectionState to)
            => CanTransition(from, to) ? to : throw new ArgumentException($"Can't transition from { from } to { to }");

        private static bool CanTransition(ConnectionState from, ConnectionState to) => _transitions[from].Contains(to);
    }
}