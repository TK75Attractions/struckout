using System;
using System.Collections.Generic;
using System.IO;
using System.Security;
using Struckout.Domain;
using UnityEngine;

namespace Struckout.Infrastructure
{
    /// <summary>
    /// 物理座標 → 描画座標の係数 (<see cref="CollisionCoordinateTransform"/>) を、JSON ファイルに読み書きする。
    ///
    /// 係数は投影の仕方で変わるので、書き出したプレイヤーでも変えられるよう、Inspector の値を起動時に
    /// このファイルで上書きする。ファイルの場所は コマンドライン &gt; 環境変数 &gt; 既定 の順で決める。
    ///
    ///   コマンドライン: -collisionTransform C:\path\collision_transform.json
    ///   環境変数:       STRUCKOUT_COLLISION_TRANSFORM
    ///   既定:           &lt;dirs::config_dir()&gt;/struckout/projector/collision_transform.json
    ///                   (Editor と書き出したプレイヤーが同じ場所を使う)
    /// </summary>
    public static class CollisionTransformStore
    {
        public const string FileName = "collision_transform.json";
        public const string ArgName = "collisionTransform";
        public const string EnvName = "STRUCKOUT_COLLISION_TRANSFORM";

        /// <summary>この起動で使うファイルの場所。</summary>
        public static string ResolvePath() =>
            ResolvePath(ParseCommandLine(), ReadEnvironmentVariable, null);

        /// <summary>入力源を明示して解決する。テストから優先順位を確かめるために公開している。</summary>
        public static string ResolvePath(
            IReadOnlyDictionary<string, string> commandLine, Func<string, string> environment, string defaultDirectory)
        {
            if (commandLine != null && commandLine.TryGetValue(ArgName, out var fromArgs) && !string.IsNullOrWhiteSpace(fromArgs))
            {
                return fromArgs;
            }

            var fromEnv = environment?.Invoke(EnvName);
            if (!string.IsNullOrWhiteSpace(fromEnv)) return fromEnv;

            return Path.Combine(defaultDirectory ?? ProjectorPaths.ConfigDirectory, FileName);
        }

        /// <summary>
        /// ファイルがあれば読んで <paramref name="target"/> に写す。無ければ何もしない (Inspector の値のまま)。
        /// 読めなかったときや値がおかしいときも <paramref name="target"/> は変えず、理由を <paramref name="error"/> に入れる。
        /// </summary>
        /// <returns>読んで写したら true。</returns>
        public static bool TryLoadInto(CollisionCoordinateTransform target, string path, out string error)
        {
            if (target == null) throw new ArgumentNullException(nameof(target));
            error = null;
            if (!File.Exists(path)) return false;

            try
            {
                var loaded = FromJson(File.ReadAllText(path));
                target.CopyFrom(loaded);
                return true;
            }
            catch (Exception e) when (e is IOException || e is UnauthorizedAccessException || e is ArgumentException)
            {
                error = e.Message;
                return false;
            }
        }

        /// <summary>係数をファイルに書く。フォルダが無ければ作る。</summary>
        public static void Save(CollisionCoordinateTransform transform, string path)
        {
            if (transform == null) throw new ArgumentNullException(nameof(transform));
            var directory = Path.GetDirectoryName(path);
            if (!string.IsNullOrEmpty(directory)) Directory.CreateDirectory(directory);
            File.WriteAllText(path, ToJson(transform));
        }

        public static string ToJson(CollisionCoordinateTransform transform) => JsonUtility.ToJson(transform, true);

        /// <summary>JSON を読む。書かれていない項目は既定値になる。1 m あたりの値が正でなければ <see cref="ArgumentException"/>。</summary>
        public static CollisionCoordinateTransform FromJson(string json)
        {
            var result = new CollisionCoordinateTransform();
            JsonUtility.FromJsonOverwrite(json, result);
            if (!(result.pixelsPerMetreX > 0f) || !(result.pixelsPerMetreY > 0f))
            {
                throw new ArgumentException(
                    $"PixelsPerMetreX and PixelsPerMetreY must be positive (use FlipX / FlipY to reverse an axis): {result}");
            }

            return result;
        }

        private static string ReadEnvironmentVariable(string name)
        {
            try
            {
                return Environment.GetEnvironmentVariable(name);
            }
            catch (SecurityException)
            {
                return null;
            }
        }

        /// <summary>"-key value" の並びを辞書にする (<see cref="NetworkSettingsResolver"/> と同じ解釈)。</summary>
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
    }
}
