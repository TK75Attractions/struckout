using Struckout.Domain;
using System.Collections.Generic;
using System;
using System.Diagnostics;
using Tk75Attractions.Struckout.V1;

namespace Struckout.Application
{
    public class GameRuntimeState
    {
        private List<Target> _targets = new();
        public IReadOnlyList<Target> Targets => _targets;

        // 的は撃たれても消えず、一定時間だけ当たらなくなる。その解除時刻を的ごとに持つ。
        // Unity 非依存にしたいので Stopwatch を使う
        // (Environment.TickCount64 は Unity の .NET Standard 2.1 プロファイルには無い)。
        private readonly Func<long> _nowMs;
        private readonly Dictionary<Target, long> _cooldownUntilMs = new();

        public GameRuntimeState() : this(null) { }

        /// <param name="nowMs">
        /// 現在時刻をミリ秒で返す関数。null なら単調増加する内部時計を使う。
        /// テストから時間を進めたいときに差し替える。
        /// </param>
        public GameRuntimeState(Func<long> nowMs)
        {
            if (nowMs != null)
            {
                _nowMs = nowMs;
                return;
            }

            var clock = Stopwatch.StartNew();
            _nowMs = () => clock.ElapsedMilliseconds;
        }

        public bool IsCoolingDown(Target target) =>
            _cooldownUntilMs.TryGetValue(target, out long until) && _nowMs() < until;

        public void StartCooldown(Target target, float seconds)
        {
            if (seconds <= 0f) return;
            _cooldownUntilMs[target] = _nowMs() + (long)(seconds * 1000f);
        }
        /// <summary>StartGame を受け取るまでは Idle。</summary>
        public GamePhase Phase { get; private set; } = GamePhase.Idle;

        public void StartGame(Difficulty difficulty)
        {
            SetDifficulty(difficulty);
            Phase = GamePhase.Playing;
        }

        public int Score { get; private set; } = 0;
        public Difficulty Difficulty { get; private set;}

        public void SetDifficulty(Difficulty difficulty)
        {
            Difficulty = difficulty;
        }
        public void AddTargets(IReadOnlyList<Target> targets)
        {
            foreach (var target in targets)
            {
                _targets.Add(target);
            }   
        }

        public void AddScore(int score)
        {
            Score += score;
        }

        public void DecreaseScore(int score)
        {
            if (score > Score)
            {
                Score = 0;
                return;
            }
            Score -= score;
        }

        public void AddTargets(ITargetGenerator generator, int num, TargetType type)
        {
            var targets = generator.GenerateTargets(num, type, Targets);
            
            foreach (var target in targets)
            {
                if (_targets.Contains(target)) throw new Exception("Add Existing Target");
                _targets.Add(target);
            }
        }

        public void RemoveTarget(Target target)
        {
            if (!_targets.Contains(target)) throw new Exception("Remove Missing Target");
            _targets.Remove(target);
        }
    }
}