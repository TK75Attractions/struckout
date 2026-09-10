using Struckout.Application;
using Struckout.Domain;
using System;
using System.Collections.Generic;

namespace Struckout.Infrastructure
{
    public class TargetGenerator : ITargetGenerator
    {
        private readonly int _xSize = 1920;
        private readonly int _ySize = 1080;
        public IReadOnlyList<Target> GenerateTargets(int num, float size)
        {
            List<Target> result = new();
            for (int i = 0; i < num; i++)
            {
                var target = GenerateTarget(TargetType.Circle, i*50, i*50, size);
            
                result.Add(target);
            }

            return result;
        }

        public IReadOnlyList<Target> GenerateTargets(int num, TargetType type, IReadOnlyList<Target> existTarget)
        {
            List<Target> targets = new(existTarget);
            List<Target> newTargets = new();
            for (int i = 0; i < num; i++)
            {
                var target = GenerateTarget(type, targets);
                targets.Add(target);
                newTargets.Add(target);
            }
            return newTargets;
        }

        public Target GenerateTarget(TargetType type, IReadOnlyList<Target> targets)
        {
            
            float maxX = 0f;
            float maxY = 0f;
            float maxScore = 0f;
            for(int i = 1; i < 200; i++)
            {
                float x = RadicalInverse(i, 2)*_xSize;
                float y = RadicalInverse(i, 3)*_ySize;

                var score = GetScore(x, y, targets);
                if (maxScore < score)
                {
                    maxX = x;
                    maxY = y;
                    maxScore = score;
                }
            }
            if(targets.Count == 0) return GenerateTarget(type, maxX, maxY, 500);
            return GenerateTarget(type, maxX, maxY, (float)maxScore);
        }

        public Target GenerateTarget(TargetType type, float X, float Y, float size)
        {
            TargetCoordinate coordinate = new(
                X,
                Y
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
            float score = float.MaxValue;
            foreach(var target in targets)
            {
                var tempScore = GetScore(x,y, target);
                score = MathF.Min(score, tempScore);
            }
            return score;
        }

        private float GetScore(float x, float y, Target targets)
        {
            var score = MathF.Sqrt((x-targets.Coordinate.X)*(x-targets.Coordinate.X) + (y-targets.Coordinate.Y)*(y-targets.Coordinate.Y)) - targets.Radius;
            score = MathF.Min(score, x);
            score = MathF.Min(score, y);
            score = MathF.Min(score, _xSize - x);
            score = MathF.Min(score, _ySize - y);
            return (float)score;
        }

        private float RadicalInverse(int index, int baseValue)
        {
            float result = 0.0f;
            float f = 1f/baseValue;

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