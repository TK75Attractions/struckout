using System;
using System.Linq;
using UnityEditor;
using UnityEditor.Build;
using UnityEditor.Build.Reporting;

namespace Editor
{
    public static class ReleaseBuild
    {
        public static void BuildWindowsArm64()
        {
            // GameCI supplies the output path to custom build methods.
            var args = Environment.GetCommandLineArgs();
            var pathIndex = Array.IndexOf(args, "-customBuildPath");
            if (pathIndex < 0 || pathIndex + 1 >= args.Length)
                throw new BuildFailedException("Missing -customBuildPath argument.");

            EditorUserBuildSettings.SetPlatformSettings(
                BuildPipeline.GetBuildTargetName(BuildTarget.StandaloneWindows64),
                "Architecture", "ARM64");
            PlayerSettings.SetScriptingBackend(NamedBuildTarget.Standalone, ScriptingImplementation.Mono2x);

            var report = BuildPipeline.BuildPlayer(new BuildPlayerOptions
            {
                scenes = EditorBuildSettings.scenes.Where(scene => scene.enabled).Select(scene => scene.path).ToArray(),
                locationPathName = args[pathIndex + 1],
                target = BuildTarget.StandaloneWindows64,
                subtarget = (int)StandaloneBuildSubtarget.Player,
                options = BuildOptions.None,
            });
            if (report.summary.result != BuildResult.Succeeded)
                throw new BuildFailedException($"Windows ARM64 build failed: {report.summary.result}");

            // カスタムのビルドメソッドを使っている場合Build Resultを正しく出力しないとgame-ciのバリデーションで落とされる
            // https://github.com/game-ci/cli/blob/v0.1.72/src/model/unity/build-validation/unity-build-validation.ts
            Console.WriteLine("Build succeeded!");
            EditorApplication.Exit(0);
        }
    }
}
