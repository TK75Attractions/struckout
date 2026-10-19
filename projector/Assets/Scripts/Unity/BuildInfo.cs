namespace Struckout.Unity
{
    internal static partial class BuildInfo
    {
        public static string Version { get; }
        public static string CommitShortHash { get; }

        static BuildInfo()
        {
            var version = "dev";
            var shortHash = "unknown";
            ApplyGeneratedValues(ref version, ref shortHash);
            Version = version;
            CommitShortHash = shortHash;
        }

        static partial void ApplyGeneratedValues(ref string version, ref string shortHash);

        public static string Display => $"{Version} ({CommitShortHash})";
    }
}
