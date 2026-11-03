using System;
using System.Globalization;
using UnityEngine;

namespace Struckout.Unity
{
    /// <summary>
    /// Caps player rendering so that a shared PC keeps CPU and GPU time for camera analysis.
    /// Frame statistics use wall-clock time so that pauses and stalls remain visible in the log.
    /// </summary>
    public sealed class RuntimeRenderingSettings : MonoBehaviour
    {
        private const int DefaultTargetFps = 60;
        private const double StatisticsIntervalSeconds = 30;

        private double _windowStartedAt;
        private double _previousFrameAt;
        private double _maximumFrameInterval;
        private int _windowStartFrame;

        [RuntimeInitializeOnLoadMethod(RuntimeInitializeLoadType.BeforeSceneLoad)]
        private static void Initialize()
        {
            // Editor rendering belongs to the development environment, not the deployed player.
            if (UnityEngine.Application.isEditor) return;
            if (!TryResolveTargetFps(Environment.GetCommandLineArgs(), out var targetFps))
            {
                Debug.LogError("[Rendering] -targetFps には 0（無制限）または正の整数を指定してください。");
                UnityEngine.Application.Quit(1);
                return;
            }

            // A fixed cap also applies on 90/120/144 Hz displays; VSync would follow their refresh rate.
            // https://docs.unity3d.com/6000.5/Documentation/ScriptReference/Application-targetFrameRate.html
            QualitySettings.vSyncCount = 0;
            UnityEngine.Application.targetFrameRate = targetFps == 0 ? -1 : targetFps;
            Debug.Log($"[Rendering] target_fps={targetFps} vsync={QualitySettings.vSyncCount}");

            var monitor = new GameObject(nameof(RuntimeRenderingSettings));
            DontDestroyOnLoad(monitor);
            monitor.AddComponent<RuntimeRenderingSettings>();
        }

        private static bool TryResolveTargetFps(string[] args, out int targetFps)
        {
            targetFps = DefaultTargetFps;
            for (var i = 1; i < args.Length; i++)
            {
                if (!string.Equals(args[i], "-targetFps", StringComparison.OrdinalIgnoreCase)) continue;
                if (++i >= args.Length ||
                    !int.TryParse(args[i], NumberStyles.Integer, CultureInfo.InvariantCulture, out targetFps) ||
                    targetFps < 0)
                    return false;
            }
            return true;
        }

        private void Awake()
        {
            _windowStartedAt = Time.realtimeSinceStartupAsDouble;
            _previousFrameAt = _windowStartedAt;
            _windowStartFrame = Time.renderedFrameCount;
        }

        private void Update()
        {
            var now = Time.realtimeSinceStartupAsDouble;
            _maximumFrameInterval = Math.Max(_maximumFrameInterval, now - _previousFrameAt);
            _previousFrameAt = now;
            var elapsed = now - _windowStartedAt;
            if (elapsed < StatisticsIntervalSeconds) return;

            var frameCount = Time.renderedFrameCount;
            var renderedFrames = frameCount - _windowStartFrame;
            Debug.Log(FormattableString.Invariant(
                $"[Rendering] window_s={elapsed:F3} frames={renderedFrames} fps={renderedFrames / elapsed:F2} max_frame_interval_ms={_maximumFrameInterval * 1000:F2}"));
            _windowStartedAt = now;
            _windowStartFrame = frameCount;
            _maximumFrameInterval = 0;
        }
    }
}
