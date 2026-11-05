using System;
using System.Linq;
using NUnit.Framework;
using Struckout.Unity;
using UnityEngine;
using Object = UnityEngine.Object;

namespace Struckout.Tests.Tests.EditMode
{
    public class GameSoundPlayerTests
    {
        [Test]
        public void Defaults_have_distinct_bounded_non_silent_waveforms_and_only_score_loops()
        {
            var root = new GameObject("Sounds");
            AudioClip[] clips;
            try
            {
                using (var player = new GameSoundPlayer(root.transform, new GameSoundSettings()))
                {
                    var sources = root.GetComponentsInChildren<AudioSource>();
                    Assert.That(sources.Length, Is.EqualTo(Enum.GetValues(typeof(GameSound)).Length));
                    Assert.That(sources.Count(source => source.loop), Is.EqualTo(1));
                    Assert.That(sources.Single(source => source.loop).clip.name, Does.EndWith("Score"));
                    clips = sources.Select(source => source.clip).ToArray();
                    Assert.That(clips.Select(clip => clip.name).Distinct().Count(), Is.EqualTo(clips.Length));
                    foreach (var clip in clips)
                    {
                        var samples = new float[clip.samples];
                        Assert.That(clip.GetData(samples, 0), Is.True);
                        Assert.That(samples.Max(sample => Math.Abs(sample)), Is.InRange(0.1f, 0.6f));
                        Assert.That(samples[0], Is.Zero);
                        Assert.That(samples[^1], Is.Zero);
                        Assert.That(clip.length, Is.InRange(0.04f, 0.5f));
                    }
                    player.StopAll();
                }
                Assert.That(clips.All(clip => clip == null), Is.True, "Generated clips must be released with the UI.");
                Assert.That(root.transform.childCount, Is.Zero);
            }
            finally { Object.DestroyImmediate(root); }
        }

        [Test]
        public void Configured_clips_are_used_and_remain_owned_by_the_asset_provider()
        {
            var root = new GameObject("Sounds");
            var custom = AudioClip.Create("Custom start", 4410, 1, 44100, false);
            try
            {
                using (var player = new GameSoundPlayer(root.transform, new GameSoundSettings { start = custom, enabled = false }))
                {
                    Assert.That(root.GetComponentsInChildren<AudioSource>().Count(source => source.clip == custom), Is.EqualTo(1));
                    Assert.DoesNotThrow(() => player.Play(GameSound.Start));
                    Assert.That(root.GetComponentsInChildren<AudioSource>().All(source => !source.isPlaying), Is.True);
                }
                Assert.That(custom == null, Is.False);
            }
            finally { Object.DestroyImmediate(custom); Object.DestroyImmediate(root); }
        }
    }
}
