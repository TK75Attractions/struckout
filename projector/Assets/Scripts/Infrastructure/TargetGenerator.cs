using Struckout.Domain;
using System;
using System.Collections.Generic;
using Struckout.Application.Interface;
using Struckout.Domain.TargetDefinition;

namespace Struckout.Infrastructure
{
    public class TargetGenerator : ITargetGenerator
    {
        private readonly float _xSize;
        private readonly float _ySize;

        /// <summary>移動先を何回まで引き直すか。</summary>
        private const int RelocationAttempts = 64;

        // 初期配置は決定的なまま (仕様)。乱数を使うのは移動先を選ぶときだけ。
        private readonly Random _random = new();

        /// <summary>
        /// 盤面の広さは <see cref="FieldBounds"/> が持つ。既定を省略できるのは
        /// テストが引数なしで作れるようにするためで、実行時は DI が値を渡す。
        /// </summary>
        public TargetGenerator(FieldBounds field = null)
        {
            field ??= new FieldBounds();
            _xSize = field.width;
            _ySize = field.height;
        }
        public IReadOnlyList<Target> GenerateTargets(int num, TargetType type, IReadOnlyList<Target> existTarget)
        {
            List<Target> targets = new(existTarget);
            List<Target> newTargets = new();
            for (var i = 0; i < num; i++)
            {
                var target = GenerateTarget(type, targets);
                targets.Add(target);
                newTargets.Add(target);
            }
            return newTargets;
        }

        public Target GenerateTarget(TargetType type, IReadOnlyList<Target> targets)
        {

            var maxX = 0f;
            var maxY = 0f;
            var maxScore = 0f;
            for (var i = 1; i < 200; i++)
            {
                var x = RadicalInverse(i, 2) * _xSize;
                var y = RadicalInverse(i, 3) * _ySize;

                var score = GetScore(x, y, targets);
                if (maxScore < score)
                {
                    maxX = x;
                    maxY = y;
                    maxScore = score;
                }
            }
            if (targets.Count == 0) return GenerateTarget(type, maxX, maxY, 500);
            return GenerateTarget(type, maxX, maxY, maxScore);
        }

        public TargetCoordinate PickRelocation(Target target, IReadOnlyList<Target> others)
        {
            // 的が画面から出ないように、中心が取りうる範囲を半径ぶん狭めておく。
            var radius = target.Radius;
            var minX = radius;
            var maxX = _xSize - radius;
            var minY = radius;
            var maxY = _ySize - radius;

            // 画面より大きい的は動かしようがない。真ん中に置く。
            if (maxX <= minX || maxY <= minY)
            {
                return new TargetCoordinate(_xSize / 2f, _ySize / 2f);
            }

            TargetCoordinate fallback = default;
            var bestClearance = float.NegativeInfinity;

            for (var attempt = 0; attempt < RelocationAttempts; attempt++)
            {
                var x = minX + (float)_random.NextDouble() * (maxX - minX);
                var y = minY + (float)_random.NextDouble() * (maxY - minY);

                var clearance = Clearance(x, y, target, others);
                if (clearance > 0f) return new TargetCoordinate(x, y);

                // どれも重なるようなら、いちばんマシだったところに置く。
                if (clearance > bestClearance)
                {
                    bestClearance = clearance;
                    fallback = new TargetCoordinate(x, y);
                }
            }

            return fallback;
        }

        /// <summary>他の的の縁までの余白。負なら重なっている。</summary>
        private float Clearance(float x, float y, Target target, IReadOnlyList<Target> others)
        {
            var clearance = float.MaxValue;
            foreach (var other in others)
            {
                var dx = x - other.Coordinate.X;
                var dy = y - other.Coordinate.Y;
                var gap = MathF.Sqrt(dx * dx + dy * dy) - other.Radius - target.Radius;
                clearance = MathF.Min(clearance, gap);
            }
            return clearance;
        }

        public Target GenerateTarget(TargetType type, float x, float y, float size)
        {
            TargetCoordinate coordinate = new(
                x,
                y
            );

            Target target = new(
                coordinate,
                type,
                size
            );

            return target;
        }

        private float GetScore(float x, float y, IReadOnlyList<Target> targets)
        {
            var score = float.MaxValue;
            foreach (var target in targets)
            {
                var tempScore = GetScore(x, y, target);
                score = MathF.Min(score, tempScore);
            }
            return score;
        }

        private float GetScore(float x, float y, Target targets)
        {
            var score = MathF.Sqrt((x - targets.Coordinate.X) * (x - targets.Coordinate.X) + (y - targets.Coordinate.Y) * (y - targets.Coordinate.Y)) - targets.Radius;
            score = MathF.Min(score, x);
            score = MathF.Min(score, y);
            score = MathF.Min(score, _xSize - x);
            score = MathF.Min(score, _ySize - y);
            return score;
        }

        private float RadicalInverse(int index, int baseValue)
        {
            var result = 0.0f;
            var f = 1f / baseValue;

            while (index > 0)
            {
                result += f * (index % baseValue);
                index /= baseValue;
                f /= baseValue;
            }

            return result;
        }
    }
}