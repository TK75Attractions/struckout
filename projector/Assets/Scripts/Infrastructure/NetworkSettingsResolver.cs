using System;
using System.Collections.Generic;
using System.Globalization;
using System.Security;
using Struckout.Domain;
using UnityEngine;

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
    ///   -masterHost 192.168.0.11 -masterPort 5001 -connectAttempts 5
    ///
    /// 環境変数:
    ///   STRUCKOUT_NETWORK_MODE / STRUCKOUT_TRACKER_HOST / STRUCKOUT_TRACKER_PORT
    ///   STRUCKOUT_MASTER_HOST / STRUCKOUT_MASTER_PORT / STRUCKOUT_CONNECT_ATTEMPTS
    /// </summary>
    public static class NetworkSettingsResolver
    {
        public static NetworkSettings Resolve(NetworkSettings inspectorSettings)
        {
            var settings = (inspectorSettings ?? new NetworkSettings()).Clone();

            // Inspector で空にされていたり、シーンに値が入っていなかったりしたときの保険。
            var defaults = new NetworkSettings();
            if (string.IsNullOrWhiteSpace(settings.TrackerHost)) settings.TrackerHost = defaults.TrackerHost;
            if (string.IsNullOrWhiteSpace(settings.MasterHost)) settings.MasterHost = defaults.MasterHost;
            if (settings.TrackerPort <= 0) settings.TrackerPort = defaults.TrackerPort;
            if (settings.MasterPort <= 0) settings.MasterPort = defaults.MasterPort;

            var commandLine = ParseCommandLine();

            settings.Mode = ResolveMode(commandLine, settings.Mode);

            settings.TrackerHost = ResolveString(commandLine, "trackerHost", "STRUCKOUT_TRACKER_HOST", settings.TrackerHost);
            settings.TrackerPort = ResolveInt(commandLine, "trackerPort", "STRUCKOUT_TRACKER_PORT", settings.TrackerPort);

            settings.MasterHost = ResolveString(commandLine, "masterHost", "STRUCKOUT_MASTER_HOST", settings.MasterHost);
            settings.MasterPort = ResolveInt(commandLine, "masterPort", "STRUCKOUT_MASTER_PORT", settings.MasterPort);

            settings.ConnectAttempts = Math.Max(
                1,
                ResolveInt(commandLine, "connectAttempts", "STRUCKOUT_CONNECT_ATTEMPTS", settings.ConnectAttempts));

            return settings;
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

            for (int i = 0; i < args.Length - 1; i++)
            {
                if (!args[i].StartsWith("-", StringComparison.Ordinal)) continue;

                var value = args[i + 1];
                if (value.StartsWith("-", StringComparison.Ordinal)) continue;

                result[args[i].TrimStart('-')] = value;
            }

            return result;
        }

        private static NetworkMode ResolveMode(Dictionary<string, string> commandLine, NetworkMode fallback)
        {
            var raw = Lookup(commandLine, "networkMode", "STRUCKOUT_NETWORK_MODE");
            if (raw == null) return fallback;

            if (Enum.TryParse<NetworkMode>(raw, ignoreCase: true, out var mode)) return mode;

            Debug.LogWarning($"[NetworkSettings] '{raw}' is not a valid network mode; falling back to {fallback}");
            return fallback;
        }

        private static string ResolveString(
            Dictionary<string, string> commandLine, string argName, string envName, string fallback)
            => Lookup(commandLine, argName, envName) ?? fallback;

        private static int ResolveInt(
            Dictionary<string, string> commandLine, string argName, string envName, int fallback)
        {
            var raw = Lookup(commandLine, argName, envName);
            if (raw == null) return fallback;

            if (int.TryParse(raw, NumberStyles.Integer, CultureInfo.InvariantCulture, out var value)) return value;

            Debug.LogWarning($"[NetworkSettings] '{raw}' is not a valid value for {argName}; falling back to {fallback}");
            return fallback;
        }

        /// <summary>コマンドライン、環境変数の順に探す。どちらにもなければ null。</summary>
        private static string Lookup(Dictionary<string, string> commandLine, string argName, string envName)
        {
            if (commandLine.TryGetValue(argName, out var fromArgs) && !string.IsNullOrWhiteSpace(fromArgs))
            {
                return fromArgs;
            }

            try
            {
                var fromEnv = Environment.GetEnvironmentVariable(envName);
                if (!string.IsNullOrWhiteSpace(fromEnv)) return fromEnv;
            }
            catch (SecurityException)
            {
                // 環境変数が読めない環境では上書きなしとして扱う。
            }

            return null;
        }
    }
}
