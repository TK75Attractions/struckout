using System;
using System.Collections.Generic;
using UnityEngine;
using Object = UnityEngine.Object;

namespace Struckout.Unity
{
    internal enum GameSound { Countdown, Start, Hit, Combo, TimeExtension, Score }

    [Serializable]
    internal sealed class GameSoundSettings
    {
        public bool enabled = true;
        [Range(0f, 1f)] public float volume = 0.35f;
        public AudioClip countdown;
        public AudioClip start;
        public AudioClip hit;
        public AudioClip combo;
        public AudioClip timeExtension;
        public AudioClip score;
    }

    /// <summary>Owns non-spatial game cues and generated clips so scenes work without external audio assets.</summary>
    internal sealed class GameSoundPlayer : IDisposable
    {
        private const int SampleRate = 44100;
        private readonly GameSoundSettings _settings;
        private readonly GameObject _root;
        private readonly Dictionary<GameSound, AudioSource> _sources = new();
        private readonly List<AudioClip> _generated = new();

        public GameSoundPlayer(Transform parent, GameSoundSettings settings)
        {
            _settings = settings ?? throw new ArgumentNullException(nameof(settings));
            _root = new GameObject("GameSounds");
            _root.transform.SetParent(parent, false);
            foreach (GameSound sound in Enum.GetValues(typeof(GameSound)))
            {
                var source = _root.AddComponent<AudioSource>();
                source.playOnAwake = false;
                source.spatialBlend = 0;
                source.loop = sound == GameSound.Score;
                source.clip = ConfiguredClip(sound);
                if (source.clip == null)
                {
                    source.clip = CreateClip(sound);
                    _generated.Add(source.clip);
                }
                _sources.Add(sound, source);
            }
        }

        private AudioClip ConfiguredClip(GameSound sound) => sound switch
        {
            GameSound.Countdown => _settings.countdown,
            GameSound.Start => _settings.start,
            GameSound.Hit => _settings.hit,
            GameSound.Combo => _settings.combo,
            GameSound.TimeExtension => _settings.timeExtension,
            GameSound.Score => _settings.score,
            _ => throw new ArgumentOutOfRangeException(nameof(sound)),
        };

        public void Play(GameSound sound)
        {
            if (!_settings.enabled || _root == null) return;
            var source = _sources[sound];
            source.volume = _settings.volume;
            // Restart a cue instead of stacking voices when several notifications arrive together.
            source.Stop();
            source.Play();
        }

        public void Stop(GameSound sound)
        {
            if (_root != null) _sources[sound].Stop();
        }

        public void StopAll()
        {
            if (_root == null) return;
            foreach (var source in _sources.Values) source.Stop();
        }

        private static AudioClip CreateClip(GameSound sound)
        {
            var (notes, noteSeconds, gapSeconds) = sound switch
            {
                GameSound.Countdown => (new[] { 660.0 }, 0.10, 0.03),
                GameSound.Start => (new[] { 523.25, 659.25, 783.99, 1046.50 }, 0.09, 0.01),
                GameSound.Hit => (new[] { 1046.50, 1567.98 }, 0.045, 0.0),
                GameSound.Combo => (new[] { 783.99, 987.77, 1174.66 }, 0.065, 0.01),
                GameSound.TimeExtension => (new[] { 659.25, 783.99, 1046.50 }, 0.10, 0.025),
                GameSound.Score => (new[] { 1318.51 }, 0.025, 0.055),
                _ => throw new ArgumentOutOfRangeException(nameof(sound)),
            };
            var noteSamples = (int)(noteSeconds * SampleRate);
            var stride = noteSamples + (int)(gapSeconds * SampleRate);
            var samples = new float[stride * notes.Length];
            for (var note = 0; note < notes.Length; note++)
            {
                for (var i = 0; i < noteSamples; i++)
                {
                    var time = (double)i / SampleRate;
                    // Short attack and release remove clicks, including at the score loop boundary.
                    var envelope = Math.Min(1, time / 0.005)
                        * Math.Min(1, (double)(noteSamples - 1 - i) / (SampleRate * 0.015));
                    var wave = Math.Sin(2 * Math.PI * notes[note] * time)
                        + 0.15 * Math.Sin(4 * Math.PI * notes[note] * time);
                    samples[note * stride + i] = (float)(0.45 * envelope * wave);
                }
            }
            var clip = AudioClip.Create("Struckout " + sound, samples.Length, 1, SampleRate, false);
            clip.SetData(samples, 0);
            return clip;
        }

        public void Dispose()
        {
            StopAll();
            foreach (var clip in _generated) Destroy(clip);
            _generated.Clear();
            Destroy(_root);
        }

        private static void Destroy(Object value)
        {
            if (UnityEngine.Application.isPlaying) Object.Destroy(value);
            else Object.DestroyImmediate(value);
        }
    }
}
