using System;
using System.IO;
using UnityEngine;

namespace Struckout.Infrastructure
{
    /// <summary>
    /// Keeps user settings alongside launcher and ball_tracker, using the same base directory as
    /// Rust's dirs::config_dir() instead of Unity's company/product-specific persistentDataPath.
    /// </summary>
    public static class ProjectorPaths
    {
        public static string ConfigDirectory => ResolveConfigDirectory(
            UnityEngine.Application.platform, Environment.GetEnvironmentVariable,
            folder => Environment.GetFolderPath(folder, Environment.SpecialFolderOption.DoNotVerify));

        /// <summary>Accepts platform inputs explicitly so path resolution can be tested on every host OS.</summary>
        public static string ResolveConfigDirectory(
            RuntimePlatform platform, Func<string, string> environment,
            Func<Environment.SpecialFolder, string> folderPath)
        {
            string directory;
            switch (platform)
            {
                case RuntimePlatform.WindowsEditor:
                case RuntimePlatform.WindowsPlayer:
                    directory = folderPath(Environment.SpecialFolder.ApplicationData);
                    break;
                case RuntimePlatform.OSXEditor:
                case RuntimePlatform.OSXPlayer:
                    directory = Path.Combine(HomeDirectory(folderPath), "Library", "Application Support");
                    break;
                case RuntimePlatform.LinuxEditor:
                case RuntimePlatform.LinuxPlayer:
                    // Unity runs on Mono, whose SpecialFolder handling can differ from .NET's XDG support.
                    directory = environment("XDG_CONFIG_HOME");
                    if (string.IsNullOrEmpty(directory) || !Path.IsPathRooted(directory))
                        directory = Path.Combine(HomeDirectory(folderPath), ".config");
                    break;
                default:
                    throw new PlatformNotSupportedException($"設定の保存先に対応していないプラットフォームです: {platform}");
            }

            if (string.IsNullOrEmpty(directory))
                throw new InvalidOperationException("設定ディレクトリが見つかりません");

            return Path.Combine(directory, "struckout", "projector");
        }

        private static string HomeDirectory(Func<Environment.SpecialFolder, string> folderPath)
        {
            var home = folderPath(Environment.SpecialFolder.UserProfile);
            if (string.IsNullOrEmpty(home))
                throw new InvalidOperationException("ホームディレクトリが見つかりません");
            return home;
        }
    }
}
