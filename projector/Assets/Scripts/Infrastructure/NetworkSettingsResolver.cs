using System;
using System.Collections.Generic;
using System.Security;
using Struckout.Domain;
using Microsoft.Extensions.Logging;

namespace Struckout.Infrastructure
{
    /// <summary>
    /// Inspector で設定した <see cref="NetworkSettings"/> に、
    /// 環境変数とコマンドライン引数の上書きを適用する。
    ///
    /// 優先順位は コマンドライン &gt; 環境変数 &gt; Inspector。
    ///
    /// docs/machine_separation.md のとおり projector と ball_tracker が別マシンに
    /// なることがあるので、ビルド済みのプレイヤーでも接続先を変えられるようにしている。
    ///
    /// コマンドライン:
    ///   -networkMode fake -trackerHost 192.168.0.10 -trackerPort 5000
    ///   -masterHost 192.168.0.11 -masterPort 8020 -machineId 2 -connectAttempts 5
    ///
    /// 環境変数:
    ///   STRUCKOUT_NETWORK_MODE / STRUCKOUT_TRACKER_HOST / STRUCKOUT_TRACKER_PORT
    ///   STRUCKOUT_MASTER_HOST / STRUCKOUT_MASTER_PORT / STRUCKOUT_MACHINE_ID
    ///   STRUCKOUT_CONNECT_ATTEMPTS
    /// </summary>
    public sealed class NetworkSettingsResolver
    {
        private readonly ILogger<NetworkSettingsResolver> _logger;

        public NetworkSettingsResolver(ILogger<NetworkSettingsResolver> logger)
        {
            _logger = logger ?? throw new ArgumentNullException(nameof(logger));
        }

        public NetworkSettings Resolve(NetworkSettings inspectorSettings)
            => Resolve(inspectorSettings, ParseCommandLine(), ReadEnvironmentVariable);

        /// <summary>
        /// 入力源を明示して解決する。テストから優先順位を確かめるために公開している。
        /// </summary>
        /// <param name="inspectorSettings">Inspectorで設定されているNetworkSettings。</param>
        /// <param name="commandLine">"-key value" を辞書にしたもの。</param>
        /// <param name="environment">環境変数を引く関数。無ければ null を返す。</param>
        public NetworkSettings Resolve(
            NetworkSettings inspectorSettings,
            IReadOnlyDictionary<string, string> commandLine,
            Func<string, string> environment)
        {
            commandLine ??= new Dictionary<string, string>();
            environment ??= _ => null;

            var settings = (inspectorSettings ?? new NetworkSettings()).Clone();

            // Inspector で空にされていたり、シーンに値が入っていなかったりしたときの保険。
            var defaults = new NetworkSettings();
            if (string.IsNullOrWhiteSpace(settings.trackerHost)) settings.trackerHost = defaults.trackerHost;
            if (string.IsNullOrWhiteSpace(settings.masterHost)) settings.masterHost = defaults.masterHost;
            if (settings.trackerPort <= 0) settings.trackerPort = defaults.trackerPort;
            if (settings.masterPort <= 0) settings.masterPort = defaults.masterPort;

            settings.mode = ResolveMode(commandLine, environment, settings.mode);

            settings.trackerHost = ResolveString(commandLine, environment, "trackerHost", "STRUCKOUT_TRACKER_HOST", settings.trackerHost);
            settings.trackerPort = ResolveUShort(commandLine, environment, "trackerPort", "STRUCKOUT_TRACKER_PORT", settings.trackerPort);

            settings.masterHost = ResolveString(commandLine, environment, "masterHost", "STRUCKOUT_MASTER_HOST", settings.masterHost);
            settings.masterPort = ResolveUShort(commandLine, environment, "masterPort", "STRUCKOUT_MASTER_PORT", settings.masterPort);

            settings.machineId = Math.Max(
                0,
                ResolveUInt(commandLine, environment, "machineId", "STRUCKOUT_MACHINE_ID", settings.machineId));

            settings.connectAttempts = Math.Max(
                1,
                ResolveUInt(commandLine, environment, "connectAttempts", "STRUCKOUT_CONNECT_ATTEMPTS", settings.connectAttempts));

            settings.simpleMode = ResolveBool(commandLine, "simpleMode", settings.simpleMode);

            return settings;
        }

        private static string ReadEnvironmentVariable(string name)
        {
            try
            {
                return Environment.GetEnvironmentVariable(name);
            }
            catch (SecurityException)
            {
                // 環境変数が読めない環境では上書きなしとして扱う。
                return null;
            }
        }

        /// <summary>"-key value" の並びを辞書にする。値のないフラグは無視する。</summary>
        private static Dictionary<string, string> ParseCommandLine()
        {
            var result = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);

            string[] args;
            try
            {
                args = Environment.GetCommandLineArgs();
            }
            catch (NotSupportedException)
            {
                // プラットフォームによっては取得できない。その場合は上書きなし。
                return result;
            }

            for (var i = 0; i < args.Length - 1; i++)
            {
                if (!args[i].StartsWith("-", StringComparison.Ordinal)) continue;

                var value = args[i + 1];
                if (value.StartsWith("-", StringComparison.Ordinal)) continue;

                result[args[i].TrimStart('-')] = value;
            }

            return result;
        }

        private NetworkMode ResolveMode(
            IReadOnlyDictionary<string, string> commandLine, Func<string, string> environment, NetworkMode fallback)
        {
            var raw = Lookup(commandLine, environment, "networkMode", "STRUCKOUT_NETWORK_MODE");
            if (raw == null) return fallback;

            if (Enum.TryParse<NetworkMode>(raw, ignoreCase: true, out var mode)) return mode;

            _logger.LogWarning("'{Value}' is not a valid network mode; falling back to {Fallback}", raw, fallback);
            return fallback;
        }

        private static string ResolveString(
            IReadOnlyDictionary<string, string> commandLine, Func<string, string> environment,
            string argName, string envName, string fallback)
            => Lookup(commandLine, environment, argName, envName) ?? fallback;


        private ushort ResolveUShort(IReadOnlyDictionary<string, string> commandLine,
            Func<string, string> environment, string argName, string envName, ushort fallback)
        {
            var raw = Lookup(commandLine, environment, argName, envName);
            if (raw is null) return fallback;

            if (ushort.TryParse(raw, out var value)) return value;
            
            _logger.LogWarning("'{Value}' is not a valid value for {Argument}; falling back to {Fallback}", raw, argName, fallback);
            return fallback;
        }

        private uint ResolveUInt(IReadOnlyDictionary<string, string> commandLine,
            Func<string, string> environment, string argName, string envName, uint fallback)
        {
            var raw = Lookup(commandLine, environment, argName, envName);
            if (raw is null) return fallback;

            if (uint.TryParse(raw, out var value)) return value;
            
            _logger.LogWarning("'{Value}' is not a valid value for {Argument}; falling back to {Fallback}", raw, argName, fallback);
            return fallback;
        }

        private bool ResolveBool(IReadOnlyDictionary<string, string> commandLine, string argName, bool fallback)
        {
            if (!commandLine.TryGetValue(argName, out var raw)) return fallback;
            if (bool.TryParse(raw, out var value)) return value;

            _logger.LogWarning("'{Value}' is not a valid value for {Argument}; falling back to {Fallback}", raw, argName, fallback);
            return fallback;
        }

        /// <summary>コマンドライン、環境変数の順に探す。どちらにもなければ null。</summary>
        private static string Lookup(
            IReadOnlyDictionary<string, string> commandLine, Func<string, string> environment,
            string argName, string envName)
        {
            if (commandLine.TryGetValue(argName, out var fromArgs) && !string.IsNullOrWhiteSpace(fromArgs))
            {
                return fromArgs;
            }

            var fromEnv = environment(envName);
            if (!string.IsNullOrWhiteSpace(fromEnv)) return fromEnv;

            return null;
        }
    }
}
